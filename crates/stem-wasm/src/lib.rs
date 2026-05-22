//! WebAssembly bindings for the Stem playground.
//!
//! Exposes a single `render(src)` function. Returns `{ html,
//! diagnostics, stats }` for the playground to display on every
//! debounced keystroke.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::HtmlExporter;
use stem_parser::parse;
use stem_types::{default_registry, validate};

#[derive(Serialize)]
struct RenderResult {
    html: String,
    diagnostics: Vec<Diag>,
    stats: Stats,
}

#[derive(Serialize)]
struct Diag {
    severity: &'static str,
    code: String,
    message: String,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
}

#[derive(Serialize)]
struct Stats {
    /// Total node count in the document (top-level + nested).
    nodes: u32,
    /// Number of error diagnostics.
    errors: u32,
    /// Number of warning diagnostics.
    warnings: u32,
}

/// Render a Stem source string. Always returns a value — even when
/// the parser surfaces errors, the partial AST is what we render so
/// the playground stays responsive while you type.
#[wasm_bindgen]
pub fn render(src: &str) -> JsValue {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let parsed = parse(src);
    let validated = validate(&parsed.document, &default_registry());

    let mut diagnostics = Vec::with_capacity(parsed.diagnostics.len() + validated.len());
    let mut errors = 0u32;
    let mut warnings = 0u32;
    for d in parsed.diagnostics.iter().chain(validated.iter()) {
        let severity = match d.severity {
            stem_core::diagnostic::Severity::Error => {
                errors += 1;
                "error"
            }
            stem_core::diagnostic::Severity::Warning => {
                warnings += 1;
                "warning"
            }
            stem_core::diagnostic::Severity::Hint => "hint",
        };
        diagnostics.push(Diag {
            severity,
            code: d.code.to_string(),
            message: d.message.clone(),
            line: d.span.start.line,
            col: d.span.start.col,
            end_line: d.span.end.line,
            end_col: d.span.end.col,
        });
    }

    let html = HtmlExporter::fragment()
        .export(&parsed.document, &Theme::default())
        .unwrap_or_else(|e| format!("<pre>export error: {}</pre>", e));

    let stats = Stats {
        nodes: count_nodes(&parsed.document),
        errors,
        warnings,
    };

    let out = RenderResult {
        html,
        diagnostics,
        stats,
    };
    serde_wasm_bindgen::to_value(&out).unwrap_or(JsValue::NULL)
}

fn count_nodes(doc: &stem_core::ast::Document) -> u32 {
    fn count_block(b: &stem_core::ast::Block) -> u32 {
        let mut n = 1u32;
        match &b.body {
            stem_core::ast::Body::Children(kids) => {
                for k in kids {
                    n += count_block(k);
                }
            }
            stem_core::ast::Body::Text(pieces) => {
                for p in pieces {
                    if let stem_core::ast::TextPiece::Inline(inline) = p {
                        n += count_block(inline);
                    }
                }
            }
            stem_core::ast::Body::None => {}
        }
        n
    }
    doc.blocks.iter().map(count_block).sum()
}
