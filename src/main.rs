use once_cell::sync::Lazy;
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, read_to_string, File};
use std::io::Write;
use std::sync::{Arc, RwLock};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use bible_api::BibleAPI;
use bible_lsp::{append_log, BibleLSP};
use tower_lsp::lsp_types::{Position, PositionEncodingKind, Range};

mod api_wrappers;
mod autocompletion;
mod bible_api;
mod bible_formatter;
mod bible_json;
mod bible_lsp;
mod book_reference;
mod book_reference_segment;
mod re;

#[derive(Debug)]
struct Backend {
    client: Client,
    lsp: BibleLSP,
}

pub static documents: Lazy<Arc<RwLock<BTreeMap<Url, String>>>> =
    Lazy::new(|| Arc::new(RwLock::new(BTreeMap::new())));

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(
                        vec![",", ";", "-", ":", " "]
                            .into_iter()
                            .map(|ch| ch.to_string())
                            .collect(),
                    ),
                    completion_item: Some(CompletionOptionsCompletionItem {
                        label_details_support: Some(true),
                    }),
                    ..CompletionOptions::default()
                }),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some(String::from("bible_lsp")),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: String::from("Bible LSP"),
                version: Some(String::from("0.0.1α")),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let TextDocumentItem { text, uri, .. } = params.text_document;
        documents.write().unwrap().insert(uri, text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        for change in params.content_changes {
            documents.write().unwrap().insert(uri.clone(), change.text);
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let doc = params.text_document_position_params.text_document;
        let text = documents
            .read()
            .unwrap()
            .get(&doc.uri)
            .cloned()
            .expect("It should be in the map");
        let pos = params.text_document_position_params.position;
        let Some(refs) = self.lsp.find_book_references(&text) else {
            return Ok(None);
        };

        let refs = refs
            .into_iter()
            .filter(|book_ref| book_ref.range.start.line == pos.line)
            .collect::<Vec<_>>();

        if refs.len() == 1 {
            let book_ref = refs.first().unwrap();
            let hover_contents = book_ref.format(&self.lsp.api);
            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::from_markdown(hover_contents)),
                range: Some(book_ref.range),
            }));
        }

        // i could just use the one under the cursor, but i dont want to do that right now
        let hover_contents = refs
            .into_iter()
            .map(|book_ref| book_ref.format(&self.lsp.api))
            .collect::<Vec<String>>()
            .join("\n\n---\n");
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::from_markdown(hover_contents)),
            range: None,
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let doc = params.text_document_position.text_document;
        let text = documents
            .read()
            .unwrap()
            .get(&doc.uri)
            .cloned()
            .expect("It should be in the map");
        let pos = params.text_document_position.position;
        let line = text
            .lines()
            .nth(pos.line as usize)
            .expect("LSP gave bad index")
            .to_string();

        // append_log(format!("{:?}\n{:#?}", &line, pos));
        // neovim panics here
        // let text_before_cursor = &line[..(pos.character as usize)];
        let text_before_cursor = &line[..(std::cmp::min(pos.character as usize, line.len()))];
        let suggestions = self.lsp.suggest_auto_completion(text_before_cursor);
        // let mut completion_items: Vec<CompletionItem> = vec![];
        // completion_items.push(CompletionItem {
        //     ..Default::default()
        // });
        let book_match = self
            .lsp
            .api
            .book_abbreviation_regex()
            .find_iter(text_before_cursor)
            .last();
        let completion_items: Vec<CompletionItem> = suggestions
            .into_iter()
            .map(|item| {
                let label = item.label(&self.lsp.api);
                // append_log(format!("{:#?}", label));
                // append_log(format!("{:#?}\n", item));
                let text_edit = match book_match {
                    Some(m) => {
                        let start = m.start() as u32;
                        let end = start + label.len() as u32;
                        Some(CompletionTextEdit::Edit(TextEdit {
                            range: Range {
                                start: Position {
                                    line: pos.line,
                                    character: start,
                                },
                                end: Position {
                                    line: pos.line,
                                    character: end,
                                },
                            },
                            new_text: label.clone(),
                        }))
                    }
                    None => None,
                };

                // match item {
                //
                // };
                let doc_content = item.lsp_preview(&self.lsp.api);
                let sort_text = item.lsp_sort();
                CompletionItem {
                    label,
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: doc_content,
                    })),
                    text_edit,
                    kind: Some(CompletionItemKind::REFERENCE),
                    sort_text: Some(sort_text),
                    ..Default::default()
                }
            })
            .collect();
        Ok(Some(CompletionResponse::Array(completion_items)))
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        let doc = params.text_document;
        let text = documents
            .read()
            .unwrap()
            .get(&doc.uri)
            .cloned()
            .expect("It should be in the map");

        let mut diagnostics: Vec<Diagnostic> = Vec::new();

        if let Some(refs) = self.lsp.find_book_references(&text) {
            for book_ref in refs.iter() {
                let Some(message) = book_ref.format_diagnostic(&self.lsp.api) else {
                    continue;
                };
                diagnostics.push(Diagnostic {
                    range: book_ref.range,
                    severity: Some(DiagnosticSeverity::INFORMATION),
                    message,
                    ..Default::default()
                });
            }
        }

        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items: diagnostics,
                },
            }),
        ))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(None)
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let json_path = "/home/dgmastertemple/Development/rust/bible_api/esv.json";
    let lsp = BibleLSP::new(json_path);
    let (service, socket) = LspService::new(|client| Backend { client, lsp });
    Server::new(stdin, stdout, socket).serve(service).await;
}
