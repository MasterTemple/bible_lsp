use book_reference::BookReference;
use book_reference_segment::{BookRange, BookReferenceSegment, BookReferenceSegments};
use once_cell::sync::Lazy;
use serde_json::Value;
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

/// Writes contents to a persistent temporary file and returns the file URI
pub fn create_temp_file_in_memory(book_name: &str, contents: &str) -> std::io::Result<Url> {
    // Create a temporary directory using the OS's temp dir
    let temp_dir = env::temp_dir();

    // Create a unique file name (e.g., definition_temp.txt)
    let temp_file_path = temp_dir.join(format!("{book_name}.md"));

    // Create and write to the file
    let mut temp_file = File::create(&temp_file_path)?;
    write!(temp_file, "{}", contents)?;

    // Convert the file path to a URI (file://)
    let uri = Url::from_file_path(&temp_file_path).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::Other, "Failed to convert path to URI")
    })?;

    Ok(uri)
}

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
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                // inline_value_provider: Some(OneOf::Left(true)),
                // inlay_hint_provider: Some(OneOf::Left(true)),
                // code_lens_provider: Some(CodeLensOptions {
                //     resolve_provider: Some(true),
                // }),
                document_symbol_provider: Some(OneOf::Left(true)),
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

    // see /home/dgmastertemple/Development/rust/scripture_lsp/src/main.rs line 233
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
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
        let cursor = params.text_document_position_params.position.character;
        // let book_ref = if refs.first().is_some_and(|found| found.range) {
        //
        // } else {};
        let Some(book_ref) = refs
            .into_iter()
            .find(|r| r.range.start.character <= cursor && cursor <= r.range.end.character)
        else {
            return Ok(None);
        };
        let book_id = book_ref.book_id;
        let end_chapter = self
            .lsp
            .api
            .get_book_chapter_count(book_id)
            .expect("This is a valid book id");
        let end_verse = self
            .lsp
            .api
            .get_chapter_verse_count(book_id, end_chapter)
            .expect("This is a valid book and chapter");
        let whole_book = BookReference {
            book_id,
            range: book_ref.range,
            segments: BookReferenceSegments(vec![BookReferenceSegment::BookRange(BookRange {
                start_chapter: 1,
                end_chapter,
                start_verse: 1,
                end_verse,
            })]),
        };

        let book_name = self.lsp.api.get_book_name(book_id).expect("It is valid");
        let content = whole_book.format_content(&self.lsp.api);
        let file_contents = format!("### {}\n\n{}", book_name, content);
        let Some((chapter, verse)) = book_ref
            .segments
            .first()
            .map(|seg| (seg.get_starting_chapter(), seg.get_starting_verse()))
        else {
            return Ok(None);
        };
        // this would have to change when i change templating
        // let the_match = format!("[{}:{}]", chapter, verse).as_str();
        let Some(the_match) = file_contents.find(format!("[{}:{}]", chapter, verse).as_str())
        else {
            return Ok(None);
        };
        let line_number = file_contents[..=the_match]
            .chars()
            .filter(|c| *c == '\n')
            .count();

        match create_temp_file_in_memory(&book_name, file_contents.as_str()) {
            Ok(uri) => Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri,
                range: Range {
                    start: Position {
                        line: line_number as u32,
                        character: 0,
                    },
                    end: Position {
                        line: line_number as u32,
                        character: 0,
                    },
                },
            }))),
            Err(_) => Ok(None),
        }
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        // params.text_document.uri
        let doc = params.text_document;
        let uri = doc.uri.clone();
        let text = documents
            .read()
            .unwrap()
            .get(&doc.uri)
            .cloned()
            .expect("It should be in the map");
        let pos = params.range.start;
        let Some(refs) = self.lsp.find_book_references(&text) else {
            return Ok(None);
        };

        let refs = refs
            .into_iter()
            .filter(|book_ref| book_ref.range.start.line == pos.line)
            .collect::<Vec<_>>();
        // append_log(format!("{:#?}", refs));
        let mut res = CodeActionResponse::new();
        for each in refs {
            res.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: format!("Insert {}", each.full_ref_label(&self.lsp.api)),
                kind: None,
                diagnostics: None,
                edit: Some(WorkspaceEdit {
                    changes: None,
                    document_changes: Some(DocumentChanges::Edits(vec![
                        // TextDocumentEdit::new()
                        TextDocumentEdit {
                            text_document: OptionalVersionedTextDocumentIdentifier {
                                uri: uri.clone(),
                                version: None,
                            },
                            // prefix inserted content with \n so that way it works when
                            // i try inserting on the next line when i am on the last line
                            edits: vec![OneOf::Left(TextEdit {
                                range: Range {
                                    start: Position {
                                        line: pos.line,
                                        character: u32::MAX,
                                    },
                                    end: Position {
                                        line: pos.line,
                                        character: u32::MAX,
                                    },
                                },
                                new_text: each.format_insert(&self.lsp.api),
                            })],
                        },
                    ])),
                    change_annotations: None,
                }),
                command: None,
                is_preferred: None,
                disabled: None,
                data: None,
                ..Default::default()
            }));

            res.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: format!("Replace {}", each.full_ref_label(&self.lsp.api)),
                kind: None,
                diagnostics: None,
                edit: Some(WorkspaceEdit {
                    changes: None,
                    document_changes: Some(DocumentChanges::Edits(vec![
                        // TextDocumentEdit::new()
                        TextDocumentEdit {
                            text_document: OptionalVersionedTextDocumentIdentifier {
                                uri: uri.clone(),
                                version: None,
                            },
                            // this doesn't work if i am on last line
                            edits: vec![OneOf::Left(TextEdit {
                                range: Range {
                                    start: Position {
                                        line: pos.line,
                                        character: 0,
                                    },
                                    end: Position {
                                        line: pos.line,
                                        character: u32::MAX,
                                    },
                                },
                                new_text: each.format_replace(&self.lsp.api),
                            })],
                        },
                    ])),
                    change_annotations: None,
                }),
                command: None,
                is_preferred: None,
                disabled: None,
                data: None,
                ..Default::default()
            }));
        }

        Ok(Some(res))
        // Ok(None)
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        Ok(Some(vec![CodeLens {
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 0,
                },
            },
            command: Some(Command {
                title: "Code Lens Title".to_string(),
                command: "command".to_string(),
                arguments: Some(vec![Value::String(String::from("arg 1"))]),
            }),
            data: None,
        }]))
    }

    async fn inline_value(&self, params: InlineValueParams) -> Result<Option<Vec<InlineValue>>> {
        Ok(Some(vec![InlineValue::Text(InlineValueText {
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: u32::MAX,
                },
            },
            text: "Inline Value".to_string(),
        })]))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        Ok(Some(vec![
            InlayHint {
                position: Position {
                    line: 1,
                    character: u32::MAX,
                },
                // label: InlayHintLabel::String(String::from("Ephesians 1:1")),
                label: InlayHintLabel::String(String::from("Paul, an apostle of Christ Jesus by the will of God, To the saints who are in Ephesus, and are faithful in Christ Jesus:")),
                kind: None,
                text_edits: None,
                tooltip: Some(InlayHintTooltip::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: String::from("### Ephesians 1:1

[1:1] Paul, an apostle of Christ Jesus by the will of God, To the saints who are in Ephesus, and are faithful in Christ Jesus:
"),
                })),
                padding_left: Some(true),
                padding_right: Some(true),
                data: None,
            },
//             InlayHint {
//                 position: Position {
//                     line: 1,
//                     character: u32::MAX,
//                 },
//                 // label: InlayHintLabel::String(String::from("John 1:1")),
//                 label: InlayHintLabel::String(String::from("In the beginning was the Word, and the Word was with God, and the Word was God.")),
//                 kind: None,
//                 text_edits: None,
//                 tooltip: Some(InlayHintTooltip::MarkupContent(MarkupContent {
//                     kind: MarkupKind::Markdown,
//                     value: String::from(
//                         "### John 1:1
//
// [1:1] In the beginning was the Word, and the Word was with God, and the Word was God.",
//                     ),
//                 })),
//                 padding_left: Some(true),
//                 padding_right: Some(true),
//                 data: None,
//             },
        ]))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let doc = params.text_document;
        let text = documents
            .read()
            .unwrap()
            .get(&doc.uri)
            .cloned()
            .expect("It should be in the map");

        // let mut symbols: Vec<Diagnostic> = Vec::new();
        let Some(refs) = self.lsp.find_book_references(&text) else {
            return Ok(None);
        };
        let symbols = refs
            .into_iter()
            .map(|book_ref| SymbolInformation {
                name: book_ref.full_ref_label(&self.lsp.api),
                kind: SymbolKind::KEY,
                location: Location {
                    uri: doc.uri.clone(),
                    range: book_ref.range,
                },
                tags: None,
                deprecated: None,
                container_name: None,
            })
            .collect::<Vec<_>>();
        Ok(Some(DocumentSymbolResponse::Flat(symbols)))
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
