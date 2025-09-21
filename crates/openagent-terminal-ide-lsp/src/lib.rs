//! Minimal LSP (Language Server Protocol) client over stdio.
//! This is a lightweight, editor-agnostic client suitable for a terminal-first UX.
#![allow(clippy::pedantic, clippy::missing_errors_doc, clippy::must_use_candidate, clippy::unnecessary_wraps, clippy::doc_markdown)]
//! This is a lightweight, editor-agnostic client suitable for a terminal-first UX.

use anyhow::{anyhow, Result};
use lsp_types as lsp;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("process not running")]
    ProcessNotRunning,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("other: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub initialization_options: Option<Value>,
}

pub struct LspClient {
    _child: Child,
    next_id: Arc<Mutex<i64>>,
    tx: mpsc::Sender<ClientMessage>,
    _pump: JoinHandle<()>,
    pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Result<Value>>>>>,
    /// Notification receiver (publishDiagnostics, etc.)
    notify_rx: Arc<Mutex<mpsc::Receiver<LspNotification>>>,
}

enum ClientMessage {
    Request { id: i64, method: String, params: Value },
    Notification { method: String, params: Value },
}

#[derive(Debug, Clone)]
pub enum LspNotification {
    PublishDiagnostics(lsp::PublishDiagnosticsParams),
}

impl LspClient {
    pub fn start(config: &ServerConfig, root_uri: Option<lsp::Url>) -> Result<Self> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;

        let (tx, rx) = mpsc::channel::<ClientMessage>();
        let pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Result<Value>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let write = Arc::new(Mutex::new(stdin));
        let (ntx, nrx) = mpsc::channel::<LspNotification>();
        let mut pump = LspPump::new(stdout, rx, pending.clone(), write.clone(), ntx)?;
        let pump_handle = std::thread::spawn(move || pump.run());

        let this = Self {
            _child: child,
            next_id: Arc::new(Mutex::new(1)),
            tx,
            _pump: pump_handle,
            pending,
            notify_rx: Arc::new(Mutex::new(nrx)),
        };

        // Initialize
        let workspace_folders = root_uri.as_ref().map(|uri| {
            vec![lsp::WorkspaceFolder { uri: uri.clone(), name: "workspace".to_string() }]
        });
        let params = lsp::InitializeParams {
            process_id: Some(std::process::id()),
            initialization_options: config.initialization_options.clone(),
            capabilities: lsp::ClientCapabilities::default(),
            workspace_folders,
            client_info: Some(lsp::ClientInfo { name: "OpenAgent Terminal".into(), version: None }),
            ..Default::default()
        };
        let _ = this.request::<lsp::InitializeResult, _>("initialize", params)?;
        this.notify("initialized", serde_json::json!({}))?;

        Ok(this)
    }

    /// Non-blocking receive for server notifications
    pub fn try_recv_notification(&self) -> Option<LspNotification> {
        self.notify_rx.lock().try_recv().ok()
    }

    pub fn hover(&self, pos: lsp::TextDocumentPositionParams) -> Result<Option<lsp::Hover>> {
        self.request("textDocument/hover", pos)
    }

    pub fn definition(
        &self,
        pos: lsp::TextDocumentPositionParams,
    ) -> Result<lsp::GotoDefinitionResponse> {
        self.request("textDocument/definition", pos)
    }

    pub fn references(&self, params: lsp::ReferenceParams) -> Result<Vec<lsp::Location>> {
        self.request("textDocument/references", params)
    }

    pub fn rename(&self, params: lsp::RenameParams) -> Result<lsp::WorkspaceEdit> {
        self.request("textDocument/rename", params)
    }

    pub fn formatting(&self, params: lsp::DocumentFormattingParams) -> Result<Vec<lsp::TextEdit>> {
        self.request("textDocument/formatting", params)
    }

    pub fn signature_help(
        &self,
        pos: lsp::TextDocumentPositionParams,
    ) -> Result<Option<lsp::SignatureHelp>> {
        self.request("textDocument/signatureHelp", pos)
    }

    pub fn open_document(&self, uri: lsp::Url, language_id: &str, text: &str) -> Result<()> {
        let params = lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version: 1,
                text: text.to_string(),
            },
        };
        self.notify("textDocument/didOpen", params)
    }

    pub fn change_document(
        &self,
        uri: lsp::Url,
        version: i32,
        changes: Vec<lsp::TextDocumentContentChangeEvent>,
    ) -> Result<()> {
        let params = lsp::DidChangeTextDocumentParams {
            text_document: lsp::VersionedTextDocumentIdentifier { uri, version },
            content_changes: changes,
        };
        self.notify("textDocument/didChange", params)
    }

    pub fn completion(
        &self,
        pos: lsp::TextDocumentPositionParams,
    ) -> Result<lsp::CompletionResponse> {
        self.request("textDocument/completion", pos)
    }

    fn request<R: DeserializeOwned, P: Serialize>(&self, method: &str, params: P) -> Result<R> {
        let id = {
            let mut g = self.next_id.lock();
            *g += 1;
            *g
        };
        let (tx_resp, rx_resp) = mpsc::channel();
        self.pending.lock().insert(id, tx_resp);
        self.tx
            .send(ClientMessage::Request {
                id,
                method: method.to_string(),
                params: serde_json::to_value(params)?,
            })
            .map_err(|_| anyhow!("tx closed"))?;

        let val = rx_resp.recv().map_err(|_| anyhow!("rx closed"))??;
        Ok(serde_json::from_value(val)?)
    }

    fn notify<P: Serialize>(&self, method: &str, params: P) -> Result<()> {
        self.tx
            .send(ClientMessage::Notification {
                method: method.to_string(),
                params: serde_json::to_value(params)?,
            })
            .map_err(|_| anyhow!("tx closed"))
    }
}

struct LspPump {
    stdout: Option<ChildStdout>,
    rx: mpsc::Receiver<ClientMessage>,
    pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Result<Value>>>>>,
    write: Arc<Mutex<ChildStdin>>,
    notify_tx: mpsc::Sender<LspNotification>,
}

impl LspPump {
    fn new(
        stdout: ChildStdout,
        rx: mpsc::Receiver<ClientMessage>,
        pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Result<Value>>>>>,
        write: Arc<Mutex<ChildStdin>>,
        notify_tx: mpsc::Sender<LspNotification>,
    ) -> Result<Self> {
        Ok(Self { stdout: Some(stdout), rx, pending, write, notify_tx })
    }

    fn run(&mut self) {
        // Reader thread: parse Content-Length framed messages and dispatch responses.
        let (resp_tx, resp_rx) = mpsc::channel::<(i64, Value)>();
        let notify_tx_clone = self.notify_tx.clone();
        let tx_for_reader = resp_tx.clone();
        let out = self.stdout.take().expect("stdout");
        std::thread::spawn(move || {
            let mut reader = out;
            loop {
                // Simple framed reader: read headers then body
                // NOTE: Production should handle partial reads more robustly
                let mut headers = String::new();
                let mut header_buf = [0u8; 1];
                // Read until CRLF CRLF
                let mut last4 = [0u8; 4];
                headers.clear();
                loop {
                    if reader.read_exact(&mut header_buf).is_err() {
                        return;
                    }
                    headers.push(header_buf[0] as char);
                    last4.rotate_left(1);
                    last4[3] = header_buf[0];
                    if last4 == [b'\r', b'\n', b'\r', b'\n'] {
                        break;
                    }
                }
                let content_length = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("Content-Length: "))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if content_length == 0 {
                    continue;
                }
                let mut body = vec![0u8; content_length];
                if reader.read_exact(&mut body).is_err() {
                    return;
                }
                if let Ok(json) = serde_json::from_slice::<Value>(&body) {
                    if let Some(id) = json.get("id").and_then(|id| id.as_i64()) {
                        let _ = tx_for_reader.send((id, json));
                    } else if let Some(method) = json.get("method").and_then(|m| m.as_str()) {
                        if method == "textDocument/publishDiagnostics" {
                            if let Some(params) = json.get("params") {
                                if let Ok(parsed) = serde_json::from_value::<
                                    lsp::PublishDiagnosticsParams,
                                >(params.clone())
                                {
                                    let _ = notify_tx_clone
                                        .send(LspNotification::PublishDiagnostics(parsed));
                                }
                            }
                        }
                    }
                }
            }
        });

        // Write loop processes requests synchronously
        loop {
            // Drain responses
            while let Ok((id, json)) = resp_rx.try_recv() {
                if let Some(sender) = self.pending.lock().remove(&id) {
                    let result = if let Some(res) = json.get("result") {
                        Ok(res.clone())
                    } else if let Some(err) = json.get("error") {
                        Err(anyhow!(err.to_string()))
                    } else {
                        Err(anyhow!("invalid response"))
                    };
                    let _ = sender.send(result);
                }
            }
            match self.rx.recv() {
                Ok(ClientMessage::Request { id, method, params }) => {
                    let json = serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
                    let bytes = serde_json::to_vec(&json).unwrap();
                    let header = format!("Content-Length: {}\r\n\r\n", bytes.len());
                    let mut stdin = self.write.lock();
                    let _ = stdin.write_all(header.as_bytes());
                    let _ = stdin.write_all(&bytes);
                    let _ = stdin.flush();
                }
                Ok(ClientMessage::Notification { method, params }) => {
                    let json = serde_json::json!({"jsonrpc":"2.0","method":method,"params":params});
                    let bytes = serde_json::to_vec(&json).unwrap();
                    let header = format!("Content-Length: {}\r\n\r\n", bytes.len());
                    let mut stdin = self.write.lock();
                    let _ = stdin.write_all(header.as_bytes());
                    let _ = stdin.write_all(&bytes);
                    let _ = stdin.flush();
                }
                Err(_) => break,
            }
        }
    }
}
