//! Stem AST → Markdown.
//!
//! Symmetric with `stem-imports::markdown`. The supported subset is the
//! same: headings (h1..h6), paragraphs, lists (ol/ul/li), blockquote,
//! fenced code blocks, links, inline code, emphasis (italic/bold/strike).
//!
//! Anything outside that subset round-trips lossily: unknown elements
//! emit a fenced block tagged `stem` with the block's name. Tables,
//! images, presentation/sheet-specific elements aren't yet supported
//! (they'll lose information; the exporter writes a placeholder).

use std::fmt::Write;

use stem_core::ast::{Block, Body, Document, TextPiece};
use stem_core::theme::Theme;
use stem_core::Exporter;
use thiserror::Error;

#[derive(Default)]
pub struct MarkdownExporter;

impl MarkdownExporter {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Error)]
pub enum MarkdownError {
    #[error("write error: {0}")]
    Write(#[from] std::fmt::Error),
}

impl Exporter for MarkdownExporter {
    type Output = String;
    type Error = MarkdownError;
    fn export(&self, doc: &Document, _theme: &Theme) -> Result<String, MarkdownError> {
        let mut out = String::new();
        for (i, block) in doc.blocks.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            emit_block(&mut out, block, 0)?;
        }
        Ok(out)
    }
}

fn emit_block(out: &mut String, b: &Block, depth: usize) -> Result<(), std::fmt::Error> {
    match b.name.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level: usize = b.name[1..].parse().unwrap_or(1);
            write!(out, "{} ", "#".repeat(level))?;
            emit_text_body(out, b)?;
            writeln!(out)?;
        }
        "p" => {
            emit_text_body(out, b)?;
            writeln!(out)?;
        }
        "blockquote" => {
            // Prefix every emitted line with "> ". Easiest: emit into a
            // scratch buffer, then prepend.
            let mut inner = String::new();
            emit_text_body(&mut inner, b)?;
            for line in inner.lines() {
                writeln!(out, "> {}", line)?;
            }
            if inner.is_empty() {
                writeln!(out, ">")?;
            }
        }
        "code" => {
            let lang = b.prop_str("lang").unwrap_or("");
            writeln!(out, "```{}", lang)?;
            let body_text = b.plain_text().unwrap_or_default();
            for line in body_text.lines() {
                writeln!(out, "{}", line)?;
            }
            if !body_text.ends_with('\n') && !body_text.is_empty() {
                // body_text.lines() drops trailing newlines; that's OK
            }
            writeln!(out, "```")?;
        }
        "ol" | "ul" => {
            if let Body::Children(items) = &b.body {
                let ordered = b.name == "ol";
                let start: usize = b.prop_str("start").and_then(|s| s.parse().ok()).unwrap_or(1);
                for (i, item) in items.iter().enumerate() {
                    let marker = if ordered {
                        format!("{}. ", start + i)
                    } else {
                        "- ".to_string()
                    };
                    emit_list_item(out, item, depth, &marker)?;
                }
            }
        }
        "hr" => {
            writeln!(out, "---")?;
        }
        _ => {
            // Unknown block: round-trip as a fenced block tagged `stem`.
            // Preserves the source name so manual editors can repair.
            writeln!(out, "```stem")?;
            writeln!(out, "{}", b.name)?;
            writeln!(out, "```")?;
        }
    }
    Ok(())
}

fn emit_list_item(
    out: &mut String,
    item: &Block,
    depth: usize,
    marker: &str,
) -> Result<(), std::fmt::Error> {
    let indent = "  ".repeat(depth);
    write!(out, "{}{}", indent, marker)?;
    match &item.body {
        Body::Text(_) => {
            emit_text_body(out, item)?;
            writeln!(out)?;
        }
        Body::Children(children) => {
            // First child on same line if it's a paragraph-like text body,
            // otherwise a nested list-style emit.
            let mut first = true;
            for child in children {
                if first && child.name == "p" {
                    emit_text_body(out, child)?;
                    writeln!(out)?;
                    first = false;
                    continue;
                }
                first = false;
                emit_block(out, child, depth + 1)?;
            }
        }
        Body::None => {
            writeln!(out)?;
        }
    }
    Ok(())
}

fn emit_text_body(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    if let Body::Text(pieces) = &b.body {
        for piece in pieces {
            match piece {
                TextPiece::Literal { text, .. } => write!(out, "{}", text)?,
                TextPiece::Inline(inline) => emit_inline(out, inline)?,
            }
        }
    }
    Ok(())
}

fn emit_inline(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    match b.name.as_str() {
        "text" => {
            // Map style properties back to MD emphasis. Multiple properties
            // stack: bold-italic → ***x***, etc.
            let bold = b.prop_str("weight") == Some("bold");
            let italic = b.prop_str("style") == Some("italic");
            let strike = b.prop_str("decoration") == Some("strike");
            let open = format!(
                "{}{}{}",
                if strike { "~~" } else { "" },
                if bold { "**" } else { "" },
                if italic { "*" } else { "" },
            );
            let close = format!(
                "{}{}{}",
                if italic { "*" } else { "" },
                if bold { "**" } else { "" },
                if strike { "~~" } else { "" },
            );
            write!(out, "{}", open)?;
            emit_text_body(out, b)?;
            write!(out, "{}", close)?;
        }
        "code" => {
            let text = b.plain_text().unwrap_or_default();
            write!(out, "`{}`", text)?;
        }
        "link" => {
            let to = b.prop_str("to").unwrap_or("");
            write!(out, "[")?;
            emit_text_body(out, b)?;
            write!(out, "](")?;
            write!(out, "{}", to)?;
            if let Some(title) = b.prop_str("title") {
                write!(out, " \"{}\"", title)?;
            }
            write!(out, ")")?;
        }
        _ => {
            // Unknown inline: emit body text uninflected; preserves
            // content if not formatting. Better than dropping silently.
            emit_text_body(out, b)?;
        }
    }
    Ok(())
}
