//! `section` — semantic document section.
//!
//! Marker variants drive the format-specific document machinery:
//! - `section[id:toc, levels:"1-3"]` → `<nav class="stem-toc">`
//!   with one link per heading inside the requested level range.
//! - `section[id:list-of-tables]` / `[id:lot]` → `<nav class="stem-lot">`
//!   linking each table caption.
//! - `section[id:list-of-figures]` / `[id:lof]` → ditto for figures.
//!
//! All three pull the entry data from the per-document prepass
//! ([`HtmlCtx`]); the prepass also assigns the matching bookmark
//! names so the in-body heading / caption renderers emit `id="…"`
//! attributes the nav links can resolve.

use stem_core::ast::Block;

use super::super::ctx::{CaptionKind, HtmlCtx};
use super::super::{html_attr, render_children_of};
use std::fmt::Write;

pub fn render_with_ctx(
    out: &mut String,
    b: &Block,
    ctx: &HtmlCtx,
) -> Result<(), std::fmt::Error> {
    let id = b.prop_str("id");

    match id {
        Some("toc") if b.body.is_none() => return render_toc(out, b, ctx),
        Some("list-of-tables" | "lot") if b.body.is_none() => {
            return render_caption_list(out, ctx, CaptionKind::Table, "List of Tables", "stem-lot");
        }
        Some("list-of-figures" | "lof") if b.body.is_none() => {
            return render_caption_list(out, ctx, CaptionKind::Figure, "List of Figures", "stem-lof");
        }
        _ => {}
    }

    match id {
        Some(id) => writeln!(out, "<section data-id=\"{}\">", html_attr(id))?,
        None => writeln!(out, "<section>")?,
    }
    render_children_of(out, b, ctx)?;
    writeln!(out, "</section>")?;
    Ok(())
}

fn render_toc(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    let (lo, hi) = parse_levels(b.prop_str("levels")).unwrap_or((1, 3));
    writeln!(
        out,
        "<nav class=\"stem-toc\" aria-label=\"Table of contents\">"
    )?;
    writeln!(out, "<h2 class=\"stem-TOCHeading\">Table of Contents</h2>")?;
    writeln!(out, "<ul>")?;
    for a in &ctx.heading_anchors {
        if a.level < lo || a.level > hi {
            continue;
        }
        writeln!(
            out,
            "<li class=\"stem-TOC{lvl}\"><a href=\"#{anchor}\">{text}</a></li>",
            lvl = a.level,
            anchor = html_attr(&a.bookmark),
            text = super::super::html_text(&a.text),
        )?;
    }
    writeln!(out, "</ul>")?;
    writeln!(out, "</nav>")?;
    Ok(())
}

fn render_caption_list(
    out: &mut String,
    ctx: &HtmlCtx,
    kind: CaptionKind,
    label: &str,
    class: &str,
) -> Result<(), std::fmt::Error> {
    writeln!(
        out,
        "<nav class=\"{}\" aria-label=\"{}\">",
        class,
        html_attr(label)
    )?;
    writeln!(out, "<h2 class=\"stem-TOCHeading\">{}</h2>", super::super::html_text(label))?;
    writeln!(out, "<ul>")?;
    let prefix = match kind {
        CaptionKind::Table => "Table",
        CaptionKind::Figure => "Figure",
    };
    for c in &ctx.caption_anchors {
        if c.kind != kind {
            continue;
        }
        writeln!(
            out,
            "<li class=\"stem-TableofFigures\"><a href=\"#{anchor}\">{prefix} {n}. {text}</a></li>",
            anchor = html_attr(&c.bookmark),
            n = c.seq,
            text = super::super::html_text(&c.text),
        )?;
    }
    writeln!(out, "</ul>")?;
    writeln!(out, "</nav>")?;
    Ok(())
}

/// Parse a levels spec like `"1-3"` or `"2"` to an inclusive
/// (lo, hi) range.
fn parse_levels(spec: Option<&str>) -> Option<(u32, u32)> {
    let s = spec?.trim();
    if let Some((a, b)) = s.split_once('-') {
        let lo: u32 = a.trim().parse().ok()?;
        let hi: u32 = b.trim().parse().ok()?;
        if lo == 0 || hi == 0 || lo > hi {
            return None;
        }
        return Some((lo, hi));
    }
    let n: u32 = s.parse().ok()?;
    if n == 0 {
        return None;
    }
    Some((n, n))
}
