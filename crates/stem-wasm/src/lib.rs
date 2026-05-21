//! WebAssembly bindings for the Stem playground.
//!
//! Exposes two render functions for the v1 ↔ v2 grammar toggle:
//! - `render(src)`    → grammar v1 (legacy)
//! - `render_v2(src)` → grammar v2 (current spec; default in the playground)
//!
//! Both return the same `{ html, diagnostics, stats }` shape so the JS
//! side can swap between them without changing the rest of the page.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use stem_core::theme::Theme;
use stem_parser::{parse, parse_v2};
use stem_render::{HtmlRenderer, HtmlV2Renderer, Renderer};
use stem_types::{default_registry, default_registry_v2, validate, validate_v2};

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

/// v1 render (legacy grammar). Kept while we transition; will be
/// removed once v2 is fully adopted.
#[wasm_bindgen]
pub fn render(src: &str) -> JsValue {
    install_panic_hook();

    let parsed = parse(src);
    let validated = validate(&parsed.document, &default_registry());

    let mut diagnostics = Vec::with_capacity(parsed.diagnostics.len() + validated.len());
    let mut errors = 0u32;
    let mut warnings = 0u32;
    for d in parsed.diagnostics.iter().chain(validated.iter()) {
        diagnostics.push(convert_diag(d, &mut errors, &mut warnings));
    }

    let html = HtmlRenderer::fragment()
        .render(&parsed.document, &Theme::default())
        .unwrap_or_else(|e| format!("<pre>render error: {}</pre>", e));

    let stats = Stats {
        nodes: count_nodes_v1(&parsed.document),
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

/// v2 render (current grammar — `name[props](text)`, `name[props]{children}`,
/// `@`-inline, sheets with `fill`/`source`/cascade).
#[wasm_bindgen]
pub fn render_v2(src: &str) -> JsValue {
    install_panic_hook();

    let parsed = parse_v2(src);
    let validated = validate_v2(&parsed.document, &default_registry_v2());

    let mut diagnostics = Vec::with_capacity(parsed.diagnostics.len() + validated.len());
    let mut errors = 0u32;
    let mut warnings = 0u32;
    for d in parsed.diagnostics.iter().chain(validated.iter()) {
        diagnostics.push(convert_diag(d, &mut errors, &mut warnings));
    }

    let html = HtmlV2Renderer::fragment()
        .render(&parsed.document, &Theme::default())
        .unwrap_or_else(|e| format!("<pre>render error: {}</pre>", e));

    let stats = Stats {
        nodes: count_nodes_v2(&parsed.document),
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

// ---------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------

fn install_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

fn convert_diag(
    d: &stem_core::Diagnostic,
    errors: &mut u32,
    warnings: &mut u32,
) -> Diag {
    let severity = match d.severity {
        stem_core::diagnostic::Severity::Error => {
            *errors += 1;
            "error"
        }
        stem_core::diagnostic::Severity::Warning => {
            *warnings += 1;
            "warning"
        }
        stem_core::diagnostic::Severity::Hint => "hint",
    };
    Diag {
        severity,
        code: d.code.to_string(),
        message: d.message.clone(),
        line: d.span.start.line,
        col: d.span.start.col,
        end_line: d.span.end.line,
        end_col: d.span.end.col,
    }
}

fn count_nodes_v1(doc: &stem_core::ast::Document) -> u32 {
    fn count_call(c: &stem_core::ast::FunctionCall) -> u32 {
        let mut n = 1u32;
        for group in &c.args {
            for item in group {
                if let stem_core::ast::Content::Call(child) = item {
                    n += count_call(child);
                }
            }
        }
        n
    }
    let mut n = 0u32;
    for node in &doc.nodes {
        match node {
            stem_core::ast::Node::Call(c) => n += count_call(c),
            stem_core::ast::Node::Text(_) => n += 1,
        }
    }
    n
}

fn count_nodes_v2(doc: &stem_core::ast_v2::Document) -> u32 {
    fn count_block(b: &stem_core::ast_v2::Block) -> u32 {
        let mut n = 1u32;
        match &b.body {
            stem_core::ast_v2::Body::Children(kids) => {
                for k in kids {
                    n += count_block(k);
                }
            }
            stem_core::ast_v2::Body::Text(pieces) => {
                for p in pieces {
                    if let stem_core::ast_v2::TextPiece::Inline(inline) = p {
                        n += count_block(inline);
                    }
                }
            }
            stem_core::ast_v2::Body::None => {}
        }
        n
    }
    doc.blocks.iter().map(count_block).sum()
}
