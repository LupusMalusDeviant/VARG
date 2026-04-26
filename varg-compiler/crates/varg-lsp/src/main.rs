use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use std::sync::Mutex;
use std::collections::HashMap;

mod diagnostics;
mod hover;
mod completion;
mod symbols;

/// Holds per-document state (source text, parsed AST, etc.)
struct DocumentState {
    text: String,
}

struct VargLanguageServer {
    client: Client,
    documents: Mutex<HashMap<Url, DocumentState>>,
}

impl VargLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(HashMap::new()),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for VargLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("varg".to_string()),
                        inter_file_dependencies: false,
                        workspace_diagnostics: false,
                        ..Default::default()
                    },
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "varg-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Varg Language Server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();

        {
            let mut docs = self.documents.lock().unwrap();
            docs.insert(uri.clone(), DocumentState { text: text.clone() });
        }

        // Publish diagnostics on open
        let diags = diagnostics::compute_diagnostics(&text);
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();

        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text.clone();

            {
                let mut docs = self.documents.lock().unwrap();
                docs.insert(uri.clone(), DocumentState { text: text.clone() });
            }

            // Re-publish diagnostics on every change
            let diags = diagnostics::compute_diagnostics(&text);
            self.client.publish_diagnostics(uri, diags, None).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let mut docs = self.documents.lock().unwrap();
        docs.remove(&uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().unwrap();
        if let Some(doc) = docs.get(uri) {
            Ok(hover::compute_hover(&doc.text, pos))
        } else {
            Ok(None)
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let docs = self.documents.lock().unwrap();
        if let Some(doc) = docs.get(uri) {
            Ok(completion::compute_completions(&doc.text, pos))
        } else {
            Ok(None)
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().unwrap();
        if let Some(doc) = docs.get(uri) {
            if let Some(word) = symbols::word_at_position(&doc.text, pos) {
                let defs = symbols::collect_definitions(&doc.text);
                // Match exact name OR method qualified as "Agent.method"
                if let Some(def) = defs
                    .iter()
                    .find(|d| d.name == word || d.name.ends_with(&format!(".{}", word)))
                {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range: def.range,
                    })));
                }
            }
        }
        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let docs = self.documents.lock().unwrap();
        if let Some(doc) = docs.get(uri) {
            if let Some(word) = symbols::word_at_position(&doc.text, pos) {
                let refs = symbols::collect_references(&doc.text);
                let locations: Vec<Location> = refs
                    .into_iter()
                    .filter(|r| r.name == word)
                    .map(|r| Location {
                        uri: uri.clone(),
                        range: r.range,
                    })
                    .collect();
                if !locations.is_empty() {
                    return Ok(Some(locations));
                }
            }
        }
        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;

        let docs = self.documents.lock().unwrap();
        if let Some(doc) = docs.get(uri) {
            let defs = symbols::collect_definitions(&doc.text);
            let syms: Vec<SymbolInformation> = defs
                .into_iter()
                .map(|d| {
                    #[allow(deprecated)]
                    SymbolInformation {
                        name: d.name,
                        kind: d.kind,
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri: uri.clone(),
                            range: d.range,
                        },
                        container_name: None,
                    }
                })
                .collect();
            return Ok(Some(DocumentSymbolResponse::Flat(syms)));
        }
        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| VargLanguageServer::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
