//! Recursive-descent parser. See `docs/grammar.md`.
//!
//!
//! The grammar is context-free — no schema lookup happens here.
//! Diagnostics are emitted but parsing always continues; the partial
//! AST is what the LSP and renderer consume even when there are errors.

use stem_core::ast::*;
use stem_core::diagnostic::Diagnostic;
use stem_core::span::{Pos, Span};

use crate::cursor::Cursor;

#[derive(Clone, Debug, Default)]
pub struct ParseResult {
    pub document: Document,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(src: &str) -> ParseResult {
    let mut p = Parser {
        cur: Cursor::new(src),
        diags: Vec::new(),
    };
    let doc = p.parse_document();
    ParseResult {
        document: doc,
        diagnostics: p.diags,
    }
}

struct Parser<'src> {
    cur: Cursor<'src>,
    diags: Vec<Diagnostic>,
}

impl<'src> Parser<'src> {
    // -----------------------------------------------------------
    // Top level
    // -----------------------------------------------------------

    fn parse_document(&mut self) -> Document {
        self.skip_ws_and_comments();
        let metadata = if self.cur.peek() == Some(b'[') {
            self.parse_metadata_header()
        } else {
            Metadata::default()
        };
        self.skip_ws_and_comments();
        let blocks = self.parse_block_list(/* inside_block_body = */ false);
        Document { metadata, blocks }
    }

    fn parse_metadata_header(&mut self) -> Metadata {
        let start = self.cur.pos();
        self.cur.bump(); // '['
        let properties = self.parse_property_list(start);
        let end = self.cur.pos();
        Metadata {
            span: Span::new(start, end),
            properties,
        }
    }

    // -----------------------------------------------------------
    // Block lists
    // -----------------------------------------------------------

    /// Parses a sequence of blocks at top level or inside a `{...}`.
    /// In the latter case, stops at the matching `}` (caller consumes).
    fn parse_block_list(&mut self, inside_block_body: bool) -> Vec<Block> {
        let mut out = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.cur.eof() {
                break;
            }
            if inside_block_body && self.cur.peek() == Some(b'}') {
                break;
            }
            // A stray `(` at this position is the "top-level loose text"
            // bug: bare (text) without a preceding ident is illegal.
            if self.cur.peek() == Some(b'(') {
                let where_ = Span::point(self.cur.pos());
                self.diags.push(Diagnostic::error(
                    "parse.top_level_text",
                    "bare `(text)` is not legal at block position — wrap in `p(text)` or another element",
                    where_,
                ));
                // recover: skip to matching `)` or newline
                self.recover_to_newline_or_close(b')');
                continue;
            }
            // Anything else must be a block (starting with an ident).
            if !is_ident_start(self.cur.peek()) {
                let where_ = Span::point(self.cur.pos());
                let b = self.cur.peek().unwrap();
                self.diags.push(Diagnostic::error(
                    "parse.expected_block",
                    format!(
                        "expected a block (identifier) but found `{}`",
                        b as char
                    ),
                    where_,
                ));
                self.cur.bump();
                continue;
            }
            match self.parse_block(/* inline = */ false) {
                Some(b) => out.push(b),
                None => {
                    // parse_block already emitted diagnostics; bump to make progress
                    if !self.cur.eof() {
                        self.cur.bump();
                    }
                }
            }
        }
        out
    }

    // -----------------------------------------------------------
    // Block
    // -----------------------------------------------------------

    /// Parse one block at the current cursor. The cursor must be at the
    /// start of the identifier (the `@` if `inline` is true was already
    /// consumed by the inline-element entry point).
    fn parse_block(&mut self, inline: bool) -> Option<Block> {
        let start = self.cur.pos();
        let (name, name_span) = self.cur.scan_ident()?;
        let name = name.to_string();

        // Properties (optional, pre-body)
        let properties = if self.cur.peek() == Some(b'[') {
            let bracket = self.cur.pos();
            self.cur.bump();
            self.parse_property_list(bracket)
        } else {
            Vec::new()
        };

        // Body (optional, exactly one)
        let body = match self.cur.peek() {
            Some(b'(') => {
                self.cur.bump();
                self.parse_text_body(start)
            }
            Some(b'{') => {
                let open = self.cur.pos();
                self.cur.bump();
                Body::Children(self.parse_block_list(true))
                    .also_close(self, open)
            }
            _ => Body::None,
        };

        // Reject post-body properties or additional bodies
        self.check_no_trailing_brackets_or_bodies(&name);

        let end = self.cur.pos();
        let block = Block {
            name,
            name_span,
            properties,
            body,
            inline_form: inline,
            span: Span::new(start, end),
        };

        if inline {
            // §G1: an @-prefixed inline must have at least props or a body
            if block.properties.is_empty() && matches!(block.body, Body::None) {
                self.diags.push(Diagnostic::error(
                    "parse.bodyless_inline_required",
                    format!(
                        "inline `@{}` must have at least properties `[…]` or a body `(…)`",
                        block.name
                    ),
                    block.span,
                ));
            }
        }

        Some(block)
    }

    fn check_no_trailing_brackets_or_bodies(&mut self, name: &str) {
        loop {
            match self.cur.peek() {
                Some(b'[') => {
                    let where_ = Span::point(self.cur.pos());
                    self.diags.push(Diagnostic::error(
                        "parse.misplaced_properties",
                        format!(
                            "properties for `{}` must come before the body; \
                             a `[…]` after the body is not allowed",
                            name
                        ),
                        where_,
                    ));
                    // consume the brackets so we don't loop forever
                    self.cur.bump();
                    let mut depth = 1;
                    while let Some(b) = self.cur.peek() {
                        self.cur.bump();
                        if b == b'[' {
                            depth += 1;
                        } else if b == b']' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                    }
                }
                Some(b'(') | Some(b'{') => {
                    let where_ = Span::point(self.cur.pos());
                    let kind = if self.cur.peek() == Some(b'(') {
                        "text"
                    } else {
                        "block"
                    };
                    self.diags.push(Diagnostic::error(
                        "parse.multiple_bodies",
                        format!(
                            "`{}` already has a body; a second ({} body) is not allowed",
                            name, kind
                        ),
                        where_,
                    ));
                    // consume the spurious body so we don't reparse it
                    let open = self.cur.bump().unwrap();
                    let close = if open == b'(' { b')' } else { b'}' };
                    let mut depth = 1;
                    while let Some(b) = self.cur.peek() {
                        self.cur.bump();
                        if b == open {
                            depth += 1;
                        } else if b == close {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                    }
                }
                _ => break,
            }
        }
    }

    // -----------------------------------------------------------
    // Properties
    // -----------------------------------------------------------

    fn parse_property_list(&mut self, opening: Pos) -> Vec<Property> {
        let mut out = Vec::new();
        loop {
            self.cur.skip_whitespace_and_newlines();
            match self.cur.peek() {
                Some(b']') => {
                    self.cur.bump();
                    return out;
                }
                None => {
                    self.diags.push(Diagnostic::error(
                        "parse.unclosed_bracket",
                        "unclosed `[`",
                        Span::point(opening),
                    ));
                    return out;
                }
                _ => {}
            }
            match self.parse_property() {
                Some(p) => out.push(p),
                None => {
                    if self.cur.peek().is_some() {
                        self.cur.bump();
                    }
                }
            }
            self.cur.skip_whitespace_and_newlines();
            match self.cur.peek() {
                Some(b',') => {
                    self.cur.bump();
                }
                Some(b']') => {
                    self.cur.bump();
                    return out;
                }
                None => {
                    self.diags.push(Diagnostic::error(
                        "parse.unclosed_bracket",
                        "unclosed `[`",
                        Span::point(opening),
                    ));
                    return out;
                }
                _ => {
                    let where_ = Span::point(self.cur.pos());
                    self.diags.push(Diagnostic::error(
                        "parse.expected_comma_or_close",
                        "expected `,` or `]`",
                        where_,
                    ));
                    while let Some(b) = self.cur.peek() {
                        if b == b',' || b == b']' {
                            break;
                        }
                        self.cur.bump();
                    }
                }
            }
        }
    }

    fn parse_property(&mut self) -> Option<Property> {
        let (key, key_span) = match self.cur.scan_ident() {
            Some(v) => (v.0.to_string(), v.1),
            None => {
                self.diags.push(Diagnostic::error(
                    "parse.expected_ident",
                    "expected property name",
                    Span::point(self.cur.pos()),
                ));
                return None;
            }
        };
        self.cur.skip_whitespace_inline();
        if self.cur.peek() != Some(b':') {
            self.diags.push(Diagnostic::error(
                "parse.expected_colon",
                "expected `:` after property name",
                Span::point(self.cur.pos()),
            ));
            return None;
        }
        self.cur.bump();
        self.cur.skip_whitespace_inline();
        let (value, value_span) = self.parse_property_value();
        Some(Property {
            key,
            key_span,
            value,
            value_span,
        })
    }

    fn parse_property_value(&mut self) -> (PropertyValue, Span) {
        let start = self.cur.pos();
        if self.cur.peek() == Some(b'"') {
            self.cur.bump();
            let s = self.consume_quoted_string_body();
            let end = self.cur.pos();
            (PropertyValue::Quoted(s), Span::new(start, end))
        } else {
            let mut buf: Vec<u8> = Vec::new();
            while let Some(b) = self.cur.peek() {
                if b == b',' || b == b']' || b == b'\n' || b == b':' {
                    if b == b':' {
                        // Bare values can't contain ':' — error.
                        let where_ = Span::point(self.cur.pos());
                        self.diags.push(Diagnostic::error(
                            "parse.bad_property_value",
                            "bare value cannot contain `:`; wrap the value in quotes",
                            where_,
                        ));
                        // consume the rest of the value to recover
                        while let Some(b) = self.cur.peek() {
                            if b == b',' || b == b']' || b == b'\n' {
                                break;
                            }
                            self.cur.bump();
                        }
                        let end = self.cur.pos();
                        return (
                            PropertyValue::Bare(String::from_utf8_lossy(&buf).trim().into()),
                            Span::new(start, end),
                        );
                    }
                    break;
                }
                buf.push(b);
                self.cur.bump();
            }
            let end = self.cur.pos();
            (
                PropertyValue::Bare(String::from_utf8_lossy(&buf).trim().to_string()),
                Span::new(start, end),
            )
        }
    }

    /// Consume the body of a quoted string starting after the opening
    /// `"`. Handles `""` doubling for literal `"`, `\\u{N}`, `\\"`,
    /// `\\\\`, `\\n`, `\\t`, `\\r`. Returns the decoded string. UTF-8
    /// is preserved by accumulating raw bytes and converting at flush
    /// boundaries.
    fn consume_quoted_string_body(&mut self) -> String {
        let mut out = String::new();
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            match self.cur.peek() {
                Some(b'"') => {
                    self.cur.bump();
                    if self.cur.peek() == Some(b'"') {
                        // `""` doubled = literal `"`
                        self.cur.bump();
                        bytes.push(b'"');
                        continue;
                    }
                    flush_bytes(&mut bytes, &mut out);
                    return out;
                }
                Some(b'\\') => {
                    self.cur.bump();
                    flush_bytes(&mut bytes, &mut out);
                    self.consume_one_escape_into(&mut out, /* allow_paren = */ false);
                }
                Some(_) => {
                    let b = self.cur.bump().unwrap();
                    bytes.push(b);
                }
                None => {
                    self.diags.push(Diagnostic::error(
                        "parse.unterminated_string",
                        "unterminated `\"` string",
                        Span::point(self.cur.pos()),
                    ));
                    flush_bytes(&mut bytes, &mut out);
                    return out;
                }
            }
        }
    }

    fn consume_one_escape_into(&mut self, out: &mut String, allow_paren: bool) {
        let where_ = Span::point(self.cur.pos());
        match self.cur.bump() {
            Some(b'"') => out.push('"'),
            Some(b'\\') => out.push('\\'),
            Some(b'n') => out.push('\n'),
            Some(b't') => out.push('\t'),
            Some(b'r') => out.push('\r'),
            Some(b'(') if allow_paren => out.push('('),
            Some(b')') if allow_paren => out.push(')'),
            Some(b'@') if allow_paren => out.push('@'),
            Some(b'u') => {
                if let Some(ch) = self.consume_unicode_escape(where_) {
                    out.push(ch);
                }
            }
            Some(other) => {
                self.diags.push(Diagnostic::error(
                    "parse.bad_escape",
                    format!("unknown escape `\\{}`", other as char),
                    where_,
                ));
                out.push(other as char);
            }
            None => {
                self.diags.push(Diagnostic::error(
                    "parse.bad_escape",
                    "unterminated escape sequence",
                    where_,
                ));
            }
        }
    }

    /// Consume the `{XXXX}` portion of `\u{XXXX}`. `where_` is the
    /// span of the `u`.
    fn consume_unicode_escape(&mut self, where_: Span) -> Option<char> {
        if self.cur.peek() != Some(b'{') {
            self.diags.push(Diagnostic::error(
                "parse.bad_escape",
                "`\\u` must be followed by `{XXXX}`",
                where_,
            ));
            return None;
        }
        self.cur.bump(); // '{'
        let mut hex = String::new();
        loop {
            match self.cur.peek() {
                Some(b'}') => {
                    self.cur.bump();
                    break;
                }
                Some(b) if b.is_ascii_hexdigit() => {
                    self.cur.bump();
                    hex.push(b as char);
                    if hex.len() > 6 {
                        self.diags.push(Diagnostic::error(
                            "parse.invalid_codepoint",
                            "Unicode escape has too many hex digits (max 6)",
                            where_,
                        ));
                        return None;
                    }
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        "parse.bad_escape",
                        "expected hex digits inside `\\u{…}`",
                        where_,
                    ));
                    return None;
                }
            }
        }
        if hex.is_empty() {
            self.diags.push(Diagnostic::error(
                "parse.bad_escape",
                "empty Unicode escape `\\u{}`",
                where_,
            ));
            return None;
        }
        let value = u32::from_str_radix(&hex, 16).ok()?;
        // Reject surrogate halves and out-of-range codepoints.
        if (0xD800..=0xDFFF).contains(&value) || value > 0x10FFFF {
            self.diags.push(Diagnostic::error(
                "parse.invalid_codepoint",
                format!("invalid Unicode codepoint U+{:X}", value),
                where_,
            ));
            return None;
        }
        char::from_u32(value)
    }

    // -----------------------------------------------------------
    // Text body
    // -----------------------------------------------------------

    /// Caller has consumed the opening `(`. Parses up to and consumes
    /// the matching `)`.
    fn parse_text_body(&mut self, block_start: Pos) -> Body {
        // Decide bare vs quoted by looking at the next non-trivial char.
        // Skip leading whitespace inside the body to decide; if it's a
        // `"`, this is a quoted body. Otherwise bare.
        let probe_pos = self.cur.pos();
        let mut peeked_ws = 0usize;
        while let Some(b) = self.cur.peek_at(peeked_ws) {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                peeked_ws += 1;
            } else {
                break;
            }
        }
        let is_quoted = self.cur.peek_at(peeked_ws) == Some(b'"');

        if is_quoted {
            // consume the leading whitespace + `"`
            for _ in 0..peeked_ws {
                self.cur.bump();
            }
            self.cur.bump(); // `"`
            let s = self.consume_quoted_string_body();
            // Allow trailing whitespace then `)`
            self.cur.skip_whitespace_and_newlines();
            if self.cur.peek() == Some(b')') {
                self.cur.bump();
            } else {
                self.diags.push(Diagnostic::error(
                    "parse.unclosed_paren",
                    "expected `)` after quoted text body",
                    Span::point(self.cur.pos()),
                ));
            }
            return Body::Text(vec![TextPiece::Literal {
                text: s,
                span: Span::new(probe_pos, self.cur.pos()),
            }]);
        }

        // Bare form
        let pieces = self.parse_bare_text_body_inner(block_start);
        if self.cur.peek() == Some(b')') {
            // Hint for completely-empty text body
            if pieces.is_empty() {
                let where_ = Span::new(probe_pos, self.cur.pos());
                self.diags.push(Diagnostic::hint(
                    "parse.empty_text_body",
                    "empty `()` text body — did you mean no body, or `{}` for a block body?",
                    where_,
                ));
            }
            self.cur.bump();
        } else {
            self.diags.push(Diagnostic::error(
                "parse.unclosed_paren",
                "unclosed `(` in text body",
                Span::point(block_start),
            ));
        }
        Body::Text(pieces)
    }

    /// Inner loop for a bare text body. Stops at unescaped `)` (does
    /// NOT consume) or EOF.
    fn parse_bare_text_body_inner(&mut self, _block_start: Pos) -> Vec<TextPiece> {
        let mut out = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let mut buf_start: Option<Pos> = None;
        let mut buf_end: Pos = self.cur.pos();

        let flush = |buf: &mut Vec<u8>,
                     buf_start: &mut Option<Pos>,
                     buf_end: Pos,
                     out: &mut Vec<TextPiece>,
                     diags: &mut Vec<Diagnostic>| {
            if buf.is_empty() {
                *buf_start = None;
                return;
            }
            let bytes = std::mem::take(buf);
            let span = Span::new(buf_start.take().unwrap_or(buf_end), buf_end);
            let text = match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(e) => {
                    diags.push(Diagnostic::error(
                        "parse.invalid_utf8",
                        "text contains invalid UTF-8",
                        span,
                    ));
                    String::from_utf8_lossy(e.as_bytes()).into_owned()
                }
            };
            out.push(TextPiece::Literal { text, span });
        };

        loop {
            match self.cur.peek() {
                None => break,
                Some(b')') => break,
                Some(b'\\') => {
                    let escape_start = self.cur.pos();
                    self.cur.bump();
                    if buf_start.is_none() {
                        buf_start = Some(escape_start);
                    }
                    let mut tmp = String::new();
                    self.consume_one_escape_into(&mut tmp, /* allow_paren = */ true);
                    // append tmp as utf-8 bytes
                    for ch in tmp.chars() {
                        let mut bytes = [0u8; 4];
                        for &b in ch.encode_utf8(&mut bytes).as_bytes() {
                            buf.push(b);
                        }
                    }
                    buf_end = self.cur.pos();
                }
                Some(b'@') => {
                    // possible inline element start
                    if is_ident_start(self.cur.peek_at(1)) {
                        flush(&mut buf, &mut buf_start, buf_end, &mut out, &mut self.diags);
                        // consume `@`
                        self.cur.bump();
                        if let Some(inline) = self.parse_block(/* inline = */ true) {
                            out.push(TextPiece::Inline(inline));
                        }
                        buf_end = self.cur.pos();
                    } else {
                        // bare `@` without ident — error
                        let where_ = Span::point(self.cur.pos());
                        self.diags.push(Diagnostic::error(
                            "parse.bad_escape",
                            "literal `@` in a bare text body must be escaped as `\\@`",
                            where_,
                        ));
                        self.cur.bump();
                    }
                }
                Some(b'(') => {
                    // bare `(` without `@ident` prefix — error (must escape)
                    let where_ = Span::point(self.cur.pos());
                    self.diags.push(Diagnostic::error(
                        "parse.bad_escape",
                        "literal `(` in a bare text body must be escaped as `\\(` or use quoted form",
                        where_,
                    ));
                    self.cur.bump();
                }
                Some(_) => {
                    if buf_start.is_none() {
                        buf_start = Some(self.cur.pos());
                    }
                    let b = self.cur.bump().unwrap();
                    buf.push(b);
                    buf_end = self.cur.pos();
                }
            }
        }

        flush(&mut buf, &mut buf_start, buf_end, &mut out, &mut self.diags);
        out
    }

    // -----------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------

    fn skip_ws_and_comments(&mut self) {
        loop {
            self.cur.skip_whitespace_and_newlines();
            if self.cur.peek() == Some(b'/') && self.cur.peek_at(1) == Some(b'/') {
                // line comment to EOL
                while let Some(b) = self.cur.peek() {
                    self.cur.bump();
                    if b == b'\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn recover_to_newline_or_close(&mut self, close: u8) {
        while let Some(b) = self.cur.peek() {
            if b == b'\n' || b == close {
                self.cur.bump();
                break;
            }
            self.cur.bump();
        }
    }
}

// ---------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------

fn is_ident_start(b: Option<u8>) -> bool {
    matches!(b, Some(b'a'..=b'z') | Some(b'A'..=b'Z'))
}

/// Drain a byte accumulator into a `String`, treating the bytes as
/// UTF-8 (lossy if invalid — invalid sequences become U+FFFD).
fn flush_bytes(bytes: &mut Vec<u8>, out: &mut String) {
    if bytes.is_empty() {
        return;
    }
    let drained = std::mem::take(bytes);
    let s = String::from_utf8_lossy(&drained);
    out.push_str(&s);
}

// ---------------------------------------------------------------
// Tiny trait shim to chain Body::Children with brace-close handling
// ---------------------------------------------------------------

trait BodyClose {
    fn also_close(self, parser: &mut Parser<'_>, opening: Pos) -> Body;
}

impl BodyClose for Body {
    fn also_close(self, parser: &mut Parser<'_>, opening: Pos) -> Body {
        if parser.cur.peek() == Some(b'}') {
            parser.cur.bump();
        } else {
            parser.diags.push(Diagnostic::error(
                "parse.unclosed_brace",
                "unclosed `{` in block body",
                Span::point(opening),
            ));
        }
        self
    }
}
