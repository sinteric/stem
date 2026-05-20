//! Second pass: markdown-flavored cooking of raw content runs.
//!
//! Renderers and validators that care about the "intended structure"
//! (paragraphs, headings, lists) consume the cooked form. Raw content is
//! still available on the AST for tools that want to round-trip.

use stem_core::ast::*;
use stem_core::span::{Pos, Span};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CookedDocument {
    pub metadata: Metadata,
    pub blocks: Vec<CookedBlock>,
}

/// What renderers actually walk. A `Call` here is always a *block-kind*
/// call (`CallKind::Block`); inline calls are folded into `Inline::Call`.
pub type CookedBlock = Block;

/// Cook a whole document.
pub fn cook_document(doc: &Document) -> CookedDocument {
    let blocks = cook_top_level(&doc.nodes);
    CookedDocument {
        metadata: doc.metadata.clone(),
        blocks,
    }
}

/// Cook a function call's body into a list of cooked blocks. Useful for
/// rendering the contents of `section(...)`, `slide(...)`, etc.
pub fn cook_call_content(call: &FunctionCall) -> Vec<CookedBlock> {
    cook_content(call.body())
}

/// Cook a raw `Content` slice (any function body) into cooked blocks.
pub fn cook_run(content: &[Content]) -> Vec<CookedBlock> {
    cook_content(content)
}

fn cook_top_level(nodes: &[Node]) -> Vec<CookedBlock> {
    let mut content = Vec::with_capacity(nodes.len());
    for n in nodes {
        match n {
            Node::Call(c) => content.push(Content::Call(c.clone())),
            Node::Text(t) => content.push(Content::Text(t.clone())),
        }
    }
    cook_content(&content)
}

fn cook_content(content: &[Content]) -> Vec<CookedBlock> {
    let mut out = Vec::new();
    let mut buffer: Vec<MdSlot> = Vec::new();

    for item in content {
        match item {
            Content::Call(c) if c.kind == CallKind::Block || is_known_block_call(&c.name) => {
                flush(&mut buffer, &mut out);
                out.push(Block::Call(c.clone()));
            }
            Content::Call(c) => {
                buffer.push(MdSlot::Call(c.clone()));
            }
            Content::Text(t) => {
                buffer.push(MdSlot::Text(t.clone()));
            }
        }
    }
    flush(&mut buffer, &mut out);
    out
}

/// Functions whose name implies block-level structure even if the parser
/// classified them as inline (because their body fit on one line). The
/// list is small and stable; richer block/inline routing happens at the
/// validator level via the registry's `block_preferred` flag.
fn is_known_block_call(name: &str) -> bool {
    matches!(
        name,
        "section"
            | "layout"
            | "col"
            | "table"
            | "row"
            | "slide"
            | "speaker-note"
            | "pagebreak"
            | "toc"
    )
}

#[derive(Clone, Debug)]
enum MdSlot {
    Text(TextRun),
    Call(FunctionCall),
}

fn flush(buffer: &mut Vec<MdSlot>, out: &mut Vec<CookedBlock>) {
    if buffer.is_empty() {
        return;
    }
    let lines = split_lines(buffer);
    buffer.clear();

    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if is_blank(line) {
            i += 1;
            continue;
        }
        if let Some(level) = heading_level(line) {
            // Skip the leading `#`s AND the single space that must follow.
            let runs = inlines_for_line(line, /* skip_prefix = */ level as usize + 1);
            out.push(Block::Heading {
                level,
                runs,
                span: line.span,
            });
            i += 1;
            continue;
        }
        if let Some(kind) = list_marker(line) {
            let mut items = Vec::new();
            let mut span = line.span;
            while i < lines.len() {
                let l = &lines[i];
                if let Some(k2) = list_marker(l) {
                    if k2 != kind {
                        break;
                    }
                    let runs = inlines_for_line(l, list_marker_len(l));
                    items.push(ListItem { runs, span: l.span });
                    span = span.merge(l.span);
                    i += 1;
                } else if is_blank(l) {
                    // a single blank can be tolerated between items —
                    // but treat as end for simplicity
                    break;
                } else {
                    break;
                }
            }
            out.push(Block::List { kind, items, span });
            continue;
        }
        // paragraph — gather consecutive non-blank, non-heading,
        // non-list lines
        let para_start = line.span.start;
        let mut para_end = line.span.end;
        let mut runs: Vec<Inline> = Vec::new();
        while i < lines.len() {
            let l = &lines[i];
            if is_blank(l) || heading_level(l).is_some() || list_marker(l).is_some() {
                break;
            }
            if !runs.is_empty() {
                // join with a space between joined lines
                runs.push(Inline::Text {
                    text: " ".to_string(),
                    style: TextStyle::default(),
                    span: Span::new(l.span.start, l.span.start),
                });
            }
            runs.extend(inlines_for_line(l, 0));
            para_end = l.span.end;
            i += 1;
        }
        out.push(Block::Paragraph(Paragraph {
            runs,
            span: Span::new(para_start, para_end),
        }));
    }
}

#[derive(Clone, Debug)]
struct Line {
    /// The slots that make up this logical line, in source order. Each
    /// `Text` slot's `text` is exactly one physical line's worth (no
    /// newline char). Calls are inline-kind and live mid-line.
    slots: Vec<MdSlot>,
    span: Span,
}

fn split_lines(slots: &[MdSlot]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut cur_slots: Vec<MdSlot> = Vec::new();
    let mut cur_start: Option<Pos> = None;
    let mut cur_end: Pos = Pos::default();

    for slot in slots {
        match slot {
            MdSlot::Call(c) => {
                if cur_start.is_none() {
                    cur_start = Some(c.span.start);
                }
                cur_end = c.span.end;
                cur_slots.push(MdSlot::Call(c.clone()));
            }
            MdSlot::Text(t) => {
                // split by '\n'
                let mut line_byte_start = t.span.start.byte;
                let mut line_pos_start = t.span.start;
                let mut line_line = t.span.start.line;
                let mut line_col = t.span.start.col;
                let bytes = t.text.as_bytes();
                let mut byte_off = 0usize;
                while byte_off < bytes.len() {
                    let b = bytes[byte_off];
                    if b == b'\n' {
                        // end current line, emitting accumulated text up to here
                        let text = String::from_utf8_lossy(&bytes[..byte_off]).into_owned();
                        // re-slice to only this physical line (relative to where we last cut)
                        let rel_start = line_byte_start.saturating_sub(t.span.start.byte);
                        let physical_text =
                            String::from_utf8_lossy(&bytes[rel_start..byte_off]).into_owned();
                        let pos_end =
                            Pos::new(t.span.start.byte + byte_off, line_line, line_col + (byte_off - rel_start) as u32);
                        if cur_start.is_none() {
                            cur_start = Some(line_pos_start);
                        }
                        cur_slots.push(MdSlot::Text(TextRun {
                            text: physical_text,
                            span: Span::new(line_pos_start, pos_end),
                        }));
                        cur_end = pos_end;
                        out.push(Line {
                            slots: std::mem::take(&mut cur_slots),
                            span: Span::new(cur_start.take().unwrap_or(pos_end), cur_end),
                        });
                        // start next line after the newline
                        line_byte_start = t.span.start.byte + byte_off + 1;
                        line_line += 1;
                        line_col = 1;
                        line_pos_start = Pos::new(line_byte_start, line_line, line_col);
                        let _ = text;
                    }
                    byte_off += 1;
                }
                // trailing partial line
                let rel_start = line_byte_start.saturating_sub(t.span.start.byte);
                if rel_start < bytes.len() {
                    let physical_text =
                        String::from_utf8_lossy(&bytes[rel_start..bytes.len()]).into_owned();
                    let pos_end = t.span.end;
                    if cur_start.is_none() {
                        cur_start = Some(line_pos_start);
                    }
                    cur_slots.push(MdSlot::Text(TextRun {
                        text: physical_text,
                        span: Span::new(line_pos_start, pos_end),
                    }));
                    cur_end = pos_end;
                }
            }
        }
    }
    if !cur_slots.is_empty() {
        out.push(Line {
            slots: std::mem::take(&mut cur_slots),
            span: Span::new(cur_start.unwrap_or(cur_end), cur_end),
        });
    }
    out
}

fn is_blank(line: &Line) -> bool {
    line.slots.iter().all(|s| match s {
        MdSlot::Text(t) => t.text.trim().is_empty(),
        MdSlot::Call(_) => false,
    })
}

fn line_first_text(line: &Line) -> Option<&str> {
    for s in &line.slots {
        if let MdSlot::Text(t) = s {
            return Some(&t.text);
        }
    }
    None
}

fn heading_level(line: &Line) -> Option<u8> {
    let s = line_first_text(line)?.trim_start();
    let level = s.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&level) {
        let after = s.as_bytes().get(level)?;
        if *after == b' ' || *after == b'\t' {
            return Some(level as u8);
        }
    }
    None
}

fn list_marker(line: &Line) -> Option<ListKind> {
    let s = line_first_text(line)?.trim_start();
    if s.starts_with("- ") || s.starts_with("* ") || s.starts_with("+ ") {
        return Some(ListKind::Unordered);
    }
    // very simple ordered: `N. ` for one or more digits
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && bytes.get(i) == Some(&b'.') && bytes.get(i + 1) == Some(&b' ') {
        return Some(ListKind::Ordered);
    }
    None
}

/// Length of the list marker within the *trimmed* line, e.g. 2 for "- "
/// or 3 for "10 ". The caller (`inlines_for_line`) adds leading whitespace
/// on top.
fn list_marker_len(line: &Line) -> usize {
    let raw = line_first_text(line).unwrap_or("");
    let trimmed = raw.trim_start();
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        2
    } else {
        let mut i = 0;
        let bytes = trimmed.as_bytes();
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        // "N. " — digits + "." + " "
        i + 2
    }
}

/// Build the inline runs for a line, optionally skipping a leading
/// number of bytes from the first text slot (used to strip a heading or
/// list marker before emphasis parsing).
fn inlines_for_line(line: &Line, skip_prefix: usize) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut first_text_skipped = false;
    for slot in &line.slots {
        match slot {
            MdSlot::Call(c) => out.push(Inline::Call(c.clone())),
            MdSlot::Text(t) => {
                let text_ref: &str = if !first_text_skipped {
                    first_text_skipped = true;
                    let s = t.text.as_str();
                    let trim_lead = s.trim_start().len();
                    let leading = s.len() - trim_lead;
                    let cut = (leading + skip_prefix).min(s.len());
                    &s[cut..]
                } else {
                    t.text.as_str()
                };
                emit_emphasis(text_ref, t.span, &mut out);
            }
        }
    }
    out
}

/// Lightweight markdown emphasis splitter: handles `**bold**`, `*italic*`,
/// and `` `code` ``. Nesting is not supported (rare in practice for
/// document content).
fn emit_emphasis(text: &str, span: Span, out: &mut Vec<Inline>) {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut plain_start = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        // bold first
        if b == b'*' && bytes.get(i + 1) == Some(&b'*') {
            if let Some(end) = find_close_2(bytes, i + 2, b'*') {
                flush_plain(text, plain_start, i, span, out);
                let inner = &text[i + 2..end];
                out.push(Inline::Text {
                    text: inner.to_string(),
                    style: TextStyle {
                        bold: true,
                        ..Default::default()
                    },
                    span,
                });
                i = end + 2;
                plain_start = i;
                continue;
            }
        }
        if b == b'*' {
            if let Some(end) = find_close_1(bytes, i + 1, b'*') {
                flush_plain(text, plain_start, i, span, out);
                let inner = &text[i + 1..end];
                out.push(Inline::Text {
                    text: inner.to_string(),
                    style: TextStyle {
                        italic: true,
                        ..Default::default()
                    },
                    span,
                });
                i = end + 1;
                plain_start = i;
                continue;
            }
        }
        if b == b'`' {
            if let Some(end) = find_close_1(bytes, i + 1, b'`') {
                flush_plain(text, plain_start, i, span, out);
                let inner = &text[i + 1..end];
                out.push(Inline::Text {
                    text: inner.to_string(),
                    style: TextStyle {
                        code: true,
                        ..Default::default()
                    },
                    span,
                });
                i = end + 1;
                plain_start = i;
                continue;
            }
        }
        i += 1;
    }
    flush_plain(text, plain_start, bytes.len(), span, out);
}

fn find_close_1(bytes: &[u8], from: usize, ch: u8) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == ch {
            // ensure not part of a "**" boundary if ch is '*'
            if ch == b'*' && bytes.get(i + 1) == Some(&b'*') {
                i += 2;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_close_2(bytes: &[u8], from: usize, ch: u8) -> Option<usize> {
    let mut i = from;
    while i + 1 < bytes.len() {
        if bytes[i] == ch && bytes[i + 1] == ch {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn flush_plain(text: &str, from: usize, to: usize, span: Span, out: &mut Vec<Inline>) {
    if from >= to {
        return;
    }
    let s = &text[from..to];
    if s.is_empty() {
        return;
    }
    out.push(Inline::Text {
        text: s.to_string(),
        style: TextStyle::default(),
        span,
    });
}
