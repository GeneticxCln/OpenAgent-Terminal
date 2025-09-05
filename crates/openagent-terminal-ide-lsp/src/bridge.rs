use crate::{LspClient, ServerConfig};
use anyhow::Result;
use lsp_types as lsp;
use openagent_terminal_ide_editor::EditorBuffer;
use url::Url;

/// Bridges an EditorBuffer to a running LSP client with proper document lifecycle
pub struct DocumentBridge {
    client: LspClient,
    buffer: EditorBuffer,
    uri: Url,
    language_id: String,
}

impl DocumentBridge {
    /// Create and initialize a document with the server
    pub fn open(mut client: LspClient, buffer: EditorBuffer, uri: Url, language_id: impl Into<String>) -> Result<Self> {
        let text = buffer.text();
        let lang: String = language_id.into();
        client.open_document(uri.clone(), &lang, &text)?;
        Ok(Self { client, buffer, uri, language_id: lang })
    }

    /// Notify server of full-document change (simple strategy).
    pub fn sync_full(&self) -> Result<()> {
        let text = self.buffer.text();
        let version = { self.buffer.meta.read().version };
        self.client.change_document(self.uri.clone(), version, vec![lsp::TextDocumentContentChangeEvent { range: None, range_length: None, text }])
    }

    pub fn position_from_cursor(&self) -> lsp::Position {
        let (line, character) = self.buffer.cursor_position_utf16();
        lsp::Position { line, character }
    }

    pub fn goto_definition(&self) -> Result<lsp::GotoDefinitionResponse> {
        let pos = lsp::TextDocumentPositionParams { text_document: lsp::TextDocumentIdentifier { uri: self.uri.clone() }, position: self.position_from_cursor() };
        self.client.definition(pos)
    }

    pub fn references(&self, include_declaration: bool) -> Result<Vec<lsp::Location>> {
        let pos = self.position_from_cursor();
        let params = lsp::ReferenceParams { text_document_position: lsp::TextDocumentPositionParams { text_document: lsp::TextDocumentIdentifier { uri: self.uri.clone() }, position: pos }, work_done_progress_params: Default::default(), partial_result_params: Default::default(), context: lsp::ReferenceContext { include_declaration } };
        self.client.references(params)
    }

    pub fn rename(&self, new_name: String) -> Result<lsp::WorkspaceEdit> {
        let pos = self.position_from_cursor();
        let params = lsp::RenameParams { text_document_position: lsp::TextDocumentPositionParams { text_document: lsp::TextDocumentIdentifier { uri: self.uri.clone() }, position: pos }, new_name, work_done_progress_params: Default::default() };
        self.client.rename(params)
    }

    pub fn formatting(&self) -> Result<Vec<lsp::TextEdit>> {
        let params = lsp::DocumentFormattingParams { text_document: lsp::TextDocumentIdentifier { uri: self.uri.clone() }, options: lsp::FormattingOptions { tab_size: 2, insert_spaces: true, ..Default::default() }, work_done_progress_params: Default::default() };
        self.client.formatting(params)
    }

    pub fn signature_help(&self) -> Result<Option<lsp::SignatureHelp>> {
        let pos = lsp::TextDocumentPositionParams { text_document: lsp::TextDocumentIdentifier { uri: self.uri.clone() }, position: self.position_from_cursor() };
        self.client.signature_help(pos)
    }
}
