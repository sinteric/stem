//! WebAssembly bindings for the Stem playground.
//!
//! Exposes a single function, `render(src) -> { html, diagnostics }`,
//! that takes a Stem source string and returns the rendered HTML
//! fragment plus structured diagnostics. The browser calls this on
//! every (debounced) keystroke for live preview.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use stem_core::theme::Theme;
use stem_parser::parse;
use stem_render::{HtmlRenderer, Renderer};
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

/// Render a Stem source to HTML + diagnostics.
///
/// Always returns a value — even if parsing finds errors, the renderer
/// still runs over the partial AST. This is what makes the playground
/// feel responsive while you're typing.
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

    let html = HtmlRenderer::fragment()
        .render(&parsed.document, &Theme::default())
        .unwrap_or_else(|e| format!("<pre>render error: {}</pre>", e));

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
