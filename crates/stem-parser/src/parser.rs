//! Recursive-descent parser. See `docs/grammar.md`.

use stem_core::ast::*;
use stem_core::diagnostic::Diagnostic;
use stem_core::span::{Pos, Span};

use crate::cursor::Cursor;
use crate::ParseResult;

pub(crate) fn parse(src: &str) -> ParseResult {
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

/// Mutable buffer for an in-progress text run. Bytes only — flushed into
/// a `TextRun` by `take_run` which validates UTF-8.
#[derive(Default)]
struct TextAcc {
    bytes: Vec<u8>,
    start: Option<Pos>,
    end: Pos,
}

impl TextAcc {
    fn push(&mut self, b: u8, before: Pos, after: Pos) {
        if self.start.is_none() {
            self.start = Some(before);
        }
        self.bytes.push(b);
        self.end = after;
    }

    fn mark_start(&mut self, p: Pos) {
        if self.start.is_none() {
            self.start = Some(p);
        }
    }

    fn mark_end(&mut self, p: Pos) {
        self.end = p;
    }

    fn take_run(&mut self, diags: &mut Vec<Diagnostic>) -> Option<TextRun> {
        if self.bytes.is_empty() {
            self.start = None;
            return None;
        }
        let bytes = std::mem::take(&mut self.bytes);
        let span = Span::new(self.start.take().unwrap_or(self.end), self.end);
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
        Some(TextRun { text, span })
    }
}

impl<'src> Parser<'src> {
    fn parse_document(&mut self) -> Document {
        self.cur.skip_whitespace_and_newlines();
        let metadata = if self.cur.peek() == Some(b'[') {
            self.parse_metadata_header()
        } else {
            Metadata::default()
        };
        self.cur.skip_whitespace_and_newlines();
        let nodes = self.parse_node_list(/* inside_call = */ false);
        Document { metadata, nodes }
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

    /// Parse the body of a `[ ... ]` list. Caller has already consumed `[`.
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
                let where_ = Span::point(self.cur.pos());
                self.diags.push(Diagnostic::error(
                    "parse.expected_ident",
                    "expected property name",
                    where_,
                ));
                return None;
            }
        };
        self.cur.skip_whitespace_inline();
        if self.cur.peek() != Some(b':') {
            let where_ = Span::point(self.cur.pos());
            self.diags.push(Diagnostic::error(
                "parse.expected_colon",
                "expected `:` after property name",
                where_,
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
            let mut buf: Vec<u8> = Vec::new();
            loop {
                match self.cur.peek() {
                    Some(b'"') => {
                        self.cur.bump();
                        break;
                    }
                    Some(b'\\') => {
                        self.cur.bump();
                        match self.cur.bump() {
                            Some(b'"') => buf.push(b'"'),
                            Some(b'\\') => buf.push(b'\\'),
                            Some(b'n') => buf.push(b'\n'),
                            Some(b't') => buf.push(b'\t'),
                            Some(b'r') => buf.push(b'\r'),
                            Some(other) => buf.push(other),
                            None => break,
                        }
                    }
                    Some(_) => {
                        let b = self.cur.bump().unwrap();
                        buf.push(b);
                    }
                    None => {
                        self.diags.push(Diagnostic::error(
                            "parse.unclosed_string",
                            "unterminated `\"` string",
                            Span::new(start, self.cur.pos()),
                        ));
                        break;
                    }
                }
            }
            let end = self.cur.pos();
            let s = String::from_utf8_lossy(&buf).into_owned();
            (PropertyValue::String(s), Span::new(start, end))
        } else {
            let begin = self.cur.pos().byte;
            while let Some(b) = self.cur.peek() {
                if b == b',' || b == b']' || b == b'\n' {
                    break;
                }
                self.cur.bump();
            }
            let end = self.cur.pos();
            let raw = self.cur.slice(begin, end.byte);
            let trimmed = raw.trim_end();
            (
                PropertyValue::Bare(trimmed.to_string()),
                Span::new(start, end),
            )
        }
    }

    fn parse_node_list(&mut self, inside_call: bool) -> Vec<Node> {
        let mut out = Vec::new();
        let mut acc = TextAcc::default();
        acc.end = self.cur.pos();
        let mut depth: u32 = 0;

        loop {
            if self.cur.eof() {
                break;
            }
            if inside_call && self.cur.peek() == Some(b')') && depth == 0 {
                break;
            }

            if self.cur.at_function_call() {
                if let Some(run) = acc.take_run(&mut self.diags) {
                    out.push(Node::Text(run));
                }
                let call = self.parse_function_call();
                out.push(Node::Call(call));
                continue;
            }

            if self.cur.peek() == Some(b'\\') {
                let before = self.cur.pos();
                self.cur.bump();
                acc.mark_start(before);
                match self.cur.bump() {
                    Some(b'\n') => {}
                    Some(b) => acc.bytes.push(b),
                    None => {}
                }
                acc.mark_end(self.cur.pos());
                continue;
            }

            if self.cur.peek() == Some(b'(') {
                let before = self.cur.pos();
                self.cur.bump();
                acc.push(b'(', before, self.cur.pos());
                depth += 1;
                continue;
            }
            if self.cur.peek() == Some(b')') {
                if depth > 0 {
                    let before = self.cur.pos();
                    self.cur.bump();
                    acc.push(b')', before, self.cur.pos());
                    depth -= 1;
                    continue;
                } else {
                    debug_assert!(!inside_call);
                    let where_ = Span::point(self.cur.pos());
                    self.diags.push(Diagnostic::warning(
                        "parse.stray_close_paren",
                        "stray `)` at top level — treated as literal text",
                        where_,
                    ));
                    let before = self.cur.pos();
                    self.cur.bump();
                    acc.push(b')', before, self.cur.pos());
                    continue;
                }
            }

            let before = self.cur.pos();
            let b = self.cur.bump().unwrap();
            acc.push(b, before, self.cur.pos());
        }

        if let Some(run) = acc.take_run(&mut self.diags) {
            out.push(Node::Text(run));
        }

        out
    }

    fn parse_function_call(&mut self) -> FunctionCall {
        let start = self.cur.pos();
        let (name, name_span) = self
            .cur
            .scan_ident()
            .expect("parse_function_call called without an ident");
        let name = name.to_string();

        // Consume any interleaved (props) and (arg groups) following the
        // identifier. The chain ends as soon as the next byte is neither
        // `[` nor `(`. We accept any interleaving — `name[a](b)`,
        // `name(a)[b]`, `name[a](b)[c]` — and merge all property lists.
        let mut args: Vec<Vec<Content>> = Vec::new();
        let mut properties: Vec<Property> = Vec::new();
        let mut saw_newline_any = false;

        loop {
            match self.cur.peek() {
                Some(b'(') => {
                    let open = self.cur.pos();
                    self.cur.bump();
                    let (group, saw_newline) = self.parse_content_run();
                    saw_newline_any |= saw_newline;
                    if self.cur.peek() == Some(b')') {
                        self.cur.bump();
                    } else {
                        self.diags.push(Diagnostic::error(
                            "parse.unclosed_paren",
                            format!("unclosed `(` for call `{}`", name),
                            Span::point(open),
                        ));
                        args.push(group);
                        break;
                    }
                    args.push(group);
                }
                Some(b'[') => {
                    let bracket = self.cur.pos();
                    self.cur.bump();
                    let mut props = self.parse_property_list(bracket);
                    properties.append(&mut props);
                }
                _ => break,
            }
        }

        let end = self.cur.pos();
        let kind = if saw_newline_any {
            CallKind::Block
        } else {
            CallKind::Inline
        };
        FunctionCall {
            name,
            name_span,
            kind,
            args,
            properties,
            span: Span::new(start, end),
        }
    }

    /// Parse a content run inside a function call. Returns the content list
    /// and whether a newline appeared at depth zero in the outer text portion.
    fn parse_content_run(&mut self) -> (Vec<Content>, bool) {
        let mut out = Vec::new();
        let mut acc = TextAcc::default();
        acc.end = self.cur.pos();
        let mut depth: u32 = 0;
        let mut saw_newline = false;

        loop {
            if self.cur.eof() {
                break;
            }
            if self.cur.peek() == Some(b')') && depth == 0 {
                break;
            }

            if self.cur.at_function_call() {
                if let Some(run) = acc.take_run(&mut self.diags) {
                    out.push(Content::Text(run));
                }
                let call = self.parse_function_call();
                out.push(Content::Call(call));
                continue;
            }

            if self.cur.peek() == Some(b'\\') {
                let before = self.cur.pos();
                self.cur.bump();
                acc.mark_start(before);
                match self.cur.bump() {
                    Some(b'\n') => {}
                    Some(b) => acc.bytes.push(b),
                    None => {}
                }
                acc.mark_end(self.cur.pos());
                continue;
            }

            if self.cur.peek() == Some(b'(') {
                let before = self.cur.pos();
                self.cur.bump();
                acc.push(b'(', before, self.cur.pos());
                depth += 1;
                continue;
            }
            if self.cur.peek() == Some(b')') {
                let before = self.cur.pos();
                self.cur.bump();
                acc.push(b')', before, self.cur.pos());
                depth -= 1;
                continue;
            }

            let before = self.cur.pos();
            let b = self.cur.bump().unwrap();
            if b == b'\n' && depth == 0 {
                saw_newline = true;
            }
            acc.push(b, before, self.cur.pos());
        }

        if let Some(run) = acc.take_run(&mut self.diags) {
            out.push(Content::Text(run));
        }

        (out, saw_newline)
    }
}
