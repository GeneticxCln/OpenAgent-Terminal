//! Minimal DAP (Debug Adapter Protocol) client over stdio.

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;

#[derive(Debug, thiserror::Error)]
pub enum DapError {
    #[error("process not running")] ProcessNotRunning,
    #[error("io error: {0}")] Io(#[from] std::io::Error),
    #[error("serde error: {0}")] Serde(#[from] serde_json::Error),
    #[error("other: {0}")] Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

pub struct DapClient {
    child: Child,
    next_id: Arc<Mutex<i64>>,
    tx: mpsc::Sender<ClientMessage>,
    _pump: JoinHandle<()>,
    pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Result<Value>>>>>,
    write: Arc<Mutex<ChildStdin>>,
    events_rx: Arc<Mutex<mpsc::Receiver<DapEvent>>>,
}

enum ClientMessage {
    Request { id: i64, command: String, arguments: Value, resp: mpsc::Sender<Result<Value>> },
    Event(Value),
}

#[derive(Debug, Clone)]
pub enum DapEvent {
    Stopped(Value),
    Continued(Value),
    Output(String),
    Thread(Value),
    Initialized,
    Terminated,
    Unknown(Value),
}

impl DapClient {
    pub fn start(config: &AdapterConfig) -> Result<Self> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;

        let (tx, rx) = mpsc::channel::<ClientMessage>();
        let pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Result<Value>>>>> = Arc::new(Mutex::new(HashMap::new()));
        let write = Arc::new(Mutex::new(stdin));
        let (etx, erx) = mpsc::channel::<DapEvent>();
        let mut pump = DapPump::new(stdout, rx, pending.clone(), write.clone(), etx)?;
        let pump_handle = std::thread::spawn(move || pump.run());

        Ok(Self {
            child,
            next_id: Arc::new(Mutex::new(1)),
            tx,
            _pump: pump_handle,
            pending,
            write,
            events_rx: Arc::new(Mutex::new(erx)),
        })
    }

    pub fn initialize(&self) -> Result<Value> {
        #[derive(Serialize)]
        struct InitArgs {
            clientID: String,
            adapterID: String,
            linesStartAt1: bool,
            columnsStartAt1: bool,
            pathFormat: String,
        }
        let args = InitArgs {
            clientID: "openagent-terminal".into(),
            adapterID: "generic".into(),
            linesStartAt1: true,
            columnsStartAt1: true,
            pathFormat: "path".into(),
        };
        self.request("initialize", serde_json::to_value(args)?)
    }

    pub fn launch(&self, arguments: Value) -> Result<Value> {
        self.request("launch", arguments)
    }

    pub fn set_breakpoints(&self, arguments: Value) -> Result<Value> {
        self.request("setBreakpoints", arguments)
    }

    /// Convenience: set breakpoints for a file path at 1-based line numbers
    pub fn set_breakpoints_for_file<P: AsRef<Path>>(&self, file: P, lines: &[i64]) -> Result<Value> {
        let source = serde_json::json!({"path": file.as_ref().to_string_lossy()});
        let bps: Vec<Value> = lines.iter().map(|l| serde_json::json!({"line": l})).collect();
        let args = serde_json::json!({"source": source, "breakpoints": bps});
        self.set_breakpoints(args)
    }

    /// Typed helper: list threads
    pub fn list_threads(&self) -> Result<Vec<(i64, String)>> {
        let v = self.threads()?;
        let threads = v.get("body").and_then(|b| b.get("threads")).and_then(|t| t.as_array()).cloned().unwrap_or_default();
        let mut out = Vec::new();
        for t in threads { if let (Some(id), Some(name)) = (t.get("id").and_then(|x| x.as_i64()), t.get("name").and_then(|x| x.as_str())) { out.push((id, name.to_string())); } }
        Ok(out)
    }

    pub fn configuration_done(&self) -> Result<Value> {
        self.request("configurationDone", serde_json::json!({}))
    }

    pub fn continue_(&self, arguments: Value) -> Result<Value> { self.request("continue", arguments) }

    pub fn next(&self, thread_id: i64) -> Result<Value> { self.request("next", serde_json::json!({"threadId": thread_id})) }
    pub fn step_in(&self, thread_id: i64) -> Result<Value> { self.request("stepIn", serde_json::json!({"threadId": thread_id})) }
    pub fn step_out(&self, thread_id: i64) -> Result<Value> { self.request("stepOut", serde_json::json!({"threadId": thread_id})) }
    pub fn pause(&self, thread_id: i64) -> Result<Value> { self.request("pause", serde_json::json!({"threadId": thread_id})) }
    pub fn disconnect(&self) -> Result<Value> { self.request("disconnect", serde_json::json!({})) }
    pub fn threads(&self) -> Result<Value> { self.request("threads", serde_json::json!({})) }
    pub fn stack_trace(&self, thread_id: i64) -> Result<Value> { self.request("stackTrace", serde_json::json!({"threadId": thread_id, "startFrame": 0, "levels": 50})) }
    pub fn scopes(&self, frame_id: i64) -> Result<Value> { self.request("scopes", serde_json::json!({"frameId": frame_id})) }
    pub fn variables(&self, variables_reference: i64) -> Result<Value> { self.request("variables", serde_json::json!({"variablesReference": variables_reference})) }
    pub fn evaluate(&self, expr: &str, frame_id: Option<i64>) -> Result<Value> {
        let mut args = serde_json::json!({"expression": expr});
        if let Some(id) = frame_id { args["frameId"] = serde_json::json!(id); }
        self.request("evaluate", args)
    }

    pub fn try_recv_event(&self) -> Option<DapEvent> { self.events_rx.lock().try_recv().ok() }

    fn request(&self, command: &str, arguments: Value) -> Result<Value> {
        let id = {
            let mut g = self.next_id.lock();
            *g += 1;
            *g
        };
        let (tx_resp, rx_resp) = mpsc::channel();
        self.pending.lock().insert(id, tx_resp);
        self.tx.send(ClientMessage::Request {
            id,
            command: command.to_string(),
            arguments,
            resp: self.pending.lock().get(&id).unwrap().clone(),
        }).map_err(|_| anyhow!("tx closed"))?;

        let val = rx_resp.recv().map_err(|_| anyhow!("rx closed"))??;
        Ok(val)
    }
}

struct DapPump {
    stdout: Option<ChildStdout>,
    rx: mpsc::Receiver<ClientMessage>,
    pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Result<Value>>>>>,
    write: Arc<Mutex<ChildStdin>>,
    events_tx: mpsc::Sender<DapEvent>,
}

impl DapPump {
    fn new(stdout: ChildStdout, rx: mpsc::Receiver<ClientMessage>, pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Result<Value>>>>>, write: Arc<Mutex<ChildStdin>>, events_tx: mpsc::Sender<DapEvent>) -> Result<Self> {
        Ok(Self { stdout: Some(stdout), rx, pending, write, events_tx })
    }

    fn run(&mut self) {
        // Reader: parse header-framed messages
        let (resp_tx, resp_rx) = mpsc::channel::<(i64, Value)>();
        let tx_for_reader = resp_tx.clone();
        let out = self.stdout.take().expect("stdout");
        let events_tx_clone = self.events_tx.clone();
        std::thread::spawn(move || {
            let mut reader = out;
            loop {
                // Read headers until CRLF CRLF
                let mut headers = String::new();
                let mut header_buf = [0u8; 1];
                let mut last4 = [0u8; 4];
                loop {
                    if reader.read_exact(&mut header_buf).is_err() { return; }
                    headers.push(header_buf[0] as char);
                    last4.rotate_left(1);
                    last4[3] = header_buf[0];
                    if last4 == [b'\r', b'\n', b'\r', b'\n'] { break; }
                }
                let content_length = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("Content-Length: "))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if content_length == 0 { continue; }
                let mut body = vec![0u8; content_length];
                if reader.read_exact(&mut body).is_err() { return; }
                if let Ok(json) = serde_json::from_slice::<Value>(&body) {
                    if let Some(request_seq) = json.get("request_seq").and_then(|v| v.as_i64()) {
                        // Responses usually have 'request_seq' mapping back to request
                        let _ = tx_for_reader.send((request_seq, json));
                    } else if json.get("type").and_then(|t| t.as_str()) == Some("event") {
                        if let Some(ev) = json.get("event").and_then(|v| v.as_str()) {
                            match ev {
                                "stopped" => { let _ = events_tx_clone.send(DapEvent::Stopped(json.clone())); },
                                "continued" => { let _ = events_tx_clone.send(DapEvent::Continued(json.clone())); },
                                "output" => {
                                    let s = json.get("body").and_then(|b| b.get("output")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let _ = events_tx_clone.send(DapEvent::Output(s));
                                },
                                "thread" => { let _ = events_tx_clone.send(DapEvent::Thread(json.clone())); },
                                "initialized" => { let _ = events_tx_clone.send(DapEvent::Initialized); },
                                "terminated" => { let _ = events_tx_clone.send(DapEvent::Terminated); },
                                _ => { let _ = events_tx_clone.send(DapEvent::Unknown(json.clone())); },
                            }
                        }
                    }
                }
            }
        });

        loop {
            while let Ok((id, json)) = resp_rx.try_recv() {
                if let Some(chan) = self.pending.lock().remove(&id) {
                    let result = if json.get("success").and_then(|b| b.as_bool()).unwrap_or(false) { Ok(json) } else { Err(anyhow!(json.to_string())) };
                    let _ = chan.send(result);
                }
            }
            match self.rx.recv() {
                Ok(ClientMessage::Request { id, command, arguments, resp: _ }) => {
                    let json = serde_json::json!({"seq": id, "type": "request", "command": command, "arguments": arguments});
                    let bytes = serde_json::to_vec(&json).unwrap();
                    let header = format!("Content-Length: {}\r\n\r\n", bytes.len());
                    let mut stdin = self.write.lock();
                    let _ = stdin.write_all(header.as_bytes());
                    let _ = stdin.write_all(&bytes);
                    let _ = stdin.flush();
                },
                Ok(ClientMessage::Event(_)) => {},
                Err(_) => break,
            }
        }
    }
}

