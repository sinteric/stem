//! LSP backend. Holds open documents in a `DashMap` and re-parses on
//! every change (Stem is small; incremental parsing is not needed yet).

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use stem_core::ast::*;
use stem_parser::parse;
use stem_types::{default_registry, validate, DocumentType, ElementSchema, Registry};

use crate::conv::{pos_to_lsp, severity_to_lsp, span_to_range};

pub struct Backend {
    pub client: Client,
    pub docs: DashMap<Url, DocState>,
    pub registry: Registry,
}

#[derive(Clone)]
#[allow(dead_code)] // text + diagnostics retained for future LSP features
pub struct DocState {
    pub text: String,
    pub doc: Document,
    pub doc_type: DocumentType,
    pub diagnostics: Vec<stem_core::Diagnostic>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            docs: DashMap::new(),
            registry: default_registry(),
        }
    }

    async fn analyze_and_publish(&self, uri: Url, text: String, version: Option<i32>) {
        let parse_result = parse(&text);
        let validation = validate(&parse_result.document, &self.registry);

        let mut all_diags = parse_result.diagnostics.clone();
        all_diags.extend(validation);

        let doc_type = parse_result
            .document
            .metadata
            .get_str("type")
            .and_then(|s| self.registry.resolve_doc_type(s))
            .unwrap_or(DocumentType::Document);

        let lsp_diags: Vec<Diagnostic> = all_diags
            .iter()
            .map(|d| Diagnostic {
                range: span_to_range(d.span),
                severity: Some(severity_to_lsp(d.severity)),
                code: Some(NumberOrString::String(d.code.to_string())),
                source: Some("stem".to_string()),
                message: d.message.clone(),
                ..Default::default()
            })
            .collect();

        self.docs.insert(
            uri.clone(),
            DocState {
                text,
                doc: parse_result.document,
                doc_type,
                diagnostics: all_diags,
            },
        );

        self.client
            .publish_diagnostics(uri, lsp_diags, version)
            .await;
    }

    fn doc(&self, uri: &Url) -> Option<DocState> {
        self.docs.get(uri).map(|r| r.clone())
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "stem-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "(".into(),
                        "[".into(),
                        ",".into(),
                        ":".into(),
                        "@".into(),
                    ]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                        legend: SemanticTokensLegend {
                            token_types: vec![
                                SemanticTokenType::FUNCTION,
                                SemanticTokenType::PROPERTY,
                                SemanticTokenType::STRING,
                                SemanticTokenType::KEYWORD,
                            ],
                            token_modifiers: vec![],
                        },
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        ..Default::default()
                    }),
                ),
                document_formatting_provider: Some(OneOf::Left(false)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "stem-lsp initialized")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text;
        let version = Some(params.text_document.version);
        self.analyze_and_publish(uri, text, version).await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.pop() {
            self.analyze_and_publish(
                params.text_document.uri,
                change.text,
                Some(params.text_document.version),
            )
            .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.docs.remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let state = match self.doc(uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let mut items = Vec::new();
        for name in self.registry.names_for(state.doc_type) {
            if let Some(schema) = self.registry.get(name, state.doc_type) {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(schema.doc.to_string()),
                    insert_text: Some(build_snippet(schema)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    ..Default::default()
                });
            }
        }
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let lsp_pos = params.text_document_position_params.position;
        let state = match self.doc(uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let needle = (lsp_pos.line, lsp_pos.character);
        if let Some(block) = find_block_at(&state.doc.blocks, needle) {
            if let Some(schema) = self.registry.get(&block.name, state.doc_type) {
                let mut md = String::new();
                md.push_str(&format!("**`{}`**\n\n", schema.name));
                md.push_str(schema.doc);
                if !schema.properties.is_empty() {
                    md.push_str("\n\n**Properties:**\n");
                    for p in schema.properties {
                        md.push_str(&format!(
                            "- `{}` — {} {}\n",
                            p.name,
                            p.doc,
                            if p.required { "(required)" } else { "" }
                        ));
                    }
                }
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: md,
                    }),
                    range: Some(span_to_range(block.name_span)),
                }));
            }
        }
        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let state = match self.doc(&params.text_document.uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let mut out = Vec::new();
        collect_symbols(&state.doc.blocks, &mut out);
        Ok(Some(DocumentSymbolResponse::Nested(out)))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let state = match self.doc(&params.text_document.uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let mut tokens = Vec::new();
        for p in &state.doc.metadata.properties {
            tokens.push((p.key_span, 1u32));
        }
        for block in &state.doc.blocks {
            collect_token_spans(block, &mut tokens);
        }
        tokens.sort_by(|a, b| {
            let pa = pos_to_lsp(a.0.start);
            let pb = pos_to_lsp(b.0.start);
            (pa.line, pa.character).cmp(&(pb.line, pb.character))
        });

        let mut data = Vec::with_capacity(tokens.len());
        let mut prev_line = 0u32;
        let mut prev_char = 0u32;
        for (span, ty) in tokens {
            let start = pos_to_lsp(span.start);
            let end = pos_to_lsp(span.end);
            let length = if end.line == start.line {
                end.character.saturating_sub(start.character)
            } else {
                continue;
            };
            if length == 0 {
                continue;
            }
            let delta_line = start.line.saturating_sub(prev_line);
            let delta_start = if delta_line == 0 {
                start.character.saturating_sub(prev_char)
            } else {
                start.character
            };
            data.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type: ty,
                token_modifiers_bitset: 0,
            });
            prev_line = start.line;
            prev_char = start.character;
        }

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------
// AST walkers — adapted to the canonical Block/Body/TextPiece shape
// ---------------------------------------------------------------

fn build_snippet(schema: &ElementSchema) -> String {
    // Insert: name + first body shape the schema declares (text/block/none)
    use stem_types::BodyKind;
    let mut s = schema.name.to_string();
    // Prefer a text body if both are allowed
    if schema.bodies.contains(&BodyKind::Text) {
        s.push_str("(${1:content})");
    } else if schema.bodies.contains(&BodyKind::Children) {
        s.push_str("{\n  $0\n}");
    }
    s
}

fn find_block_at(blocks: &[Block], pos: (u32, u32)) -> Option<&Block> {
    for b in blocks {
        if span_contains(b.name_span, pos) {
            return Some(b);
        }
        if let Some(inner) = find_block_in(b, pos) {
            return Some(inner);
        }
    }
    None
}

fn find_block_in(block: &Block, pos: (u32, u32)) -> Option<&Block> {
    match &block.body {
        Body::Children(kids) => find_block_at(kids, pos),
        Body::Text(pieces) => {
            for p in pieces {
                if let TextPiece::Inline(inline) = p {
                    if span_contains(inline.name_span, pos) {
                        return Some(inline);
                    }
                    if let Some(inner) = find_block_in(inline, pos) {
                        return Some(inner);
                    }
                }
            }
            None
        }
        Body::None => None,
    }
}

fn span_contains(span: stem_core::Span, pos: (u32, u32)) -> bool {
    let start = pos_to_lsp(span.start);
    let end = pos_to_lsp(span.end);
    let (l, c) = pos;
    if l < start.line || l > end.line {
        return false;
    }
    if l == start.line && c < start.character {
        return false;
    }
    if l == end.line && c > end.character {
        return false;
    }
    true
}

fn collect_symbols(blocks: &[Block], out: &mut Vec<DocumentSymbol>) {
    for b in blocks {
        #[allow(deprecated)]
        let mut sym = DocumentSymbol {
            name: b.name.clone(),
            detail: detail_for(b),
            kind: kind_for(&b.name),
            tags: None,
            deprecated: None,
            range: span_to_range(b.span),
            selection_range: span_to_range(b.name_span),
            children: Some(Vec::new()),
        };
        match &b.body {
            Body::Children(kids) => collect_symbols(kids, sym.children.as_mut().unwrap()),
            Body::Text(pieces) => {
                for p in pieces {
                    if let TextPiece::Inline(inline) = p {
                        collect_symbols(std::slice::from_ref(inline), sym.children.as_mut().unwrap());
                    }
                }
            }
            Body::None => {}
        }
        out.push(sym);
    }
}

fn detail_for(b: &Block) -> Option<String> {
    // For sheet-style cells, surface the address; for sections, the id.
    if let Some(id) = b.prop_str("id") {
        return Some(format!("[id:{}]", id));
    }
    if let Some(at) = b.prop_str("at") {
        return Some(format!("[at:{}]", at));
    }
    b.plain_text().filter(|s| !s.trim().is_empty()).map(|s| {
        let s = s.trim();
        if s.len() > 40 { format!("{}…", &s[..40]) } else { s.to_string() }
    })
}

fn kind_for(name: &str) -> SymbolKind {
    match name {
        "section" | "slide" => SymbolKind::CLASS,
        "layout" | "col" => SymbolKind::NAMESPACE,
        "table" | "row" | "cell" | "sheet" => SymbolKind::STRUCT,
        "footnote" | "note" | "speaker-note" => SymbolKind::STRING,
        _ => SymbolKind::FUNCTION,
    }
}

fn collect_token_spans(block: &Block, out: &mut Vec<(stem_core::Span, u32)>) {
    out.push((block.name_span, 0)); // FUNCTION
    for p in &block.properties {
        out.push((p.key_span, 1)); // PROPERTY
        if let stem_core::ast::PropertyValue::Quoted(_) = p.value {
            out.push((p.value_span, 2)); // STRING
        }
    }
    match &block.body {
        Body::Children(kids) => {
            for k in kids {
                collect_token_spans(k, out);
            }
        }
        Body::Text(pieces) => {
            for p in pieces {
                if let TextPiece::Inline(inline) = p {
                    collect_token_spans(inline, out);
                }
            }
        }
        Body::None => {}
    }
}
