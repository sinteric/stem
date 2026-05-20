//! A byte cursor with line/column tracking.
//!
//! We work in bytes, not chars, because Stem's significant characters
//! (`()[]:,\\`) are all ASCII. Multi-byte UTF-8 sequences are preserved
//! verbatim in text runs.

use stem_core::span::{Pos, Span};

pub(crate) struct Cursor<'src> {
    src: &'src [u8],
    pos: Pos,
}

impl<'src> Cursor<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: Pos::new(0, 1, 1),
        }
    }

    pub fn pos(&self) -> Pos {
        self.pos
    }

    pub fn eof(&self) -> bool {
        self.pos.byte >= self.src.len()
    }

    pub fn peek(&self) -> Option<u8> {
        self.src.get(self.pos.byte).copied()
    }

    /// Peek `n` bytes ahead, 0-indexed.
    pub fn peek_at(&self, n: usize) -> Option<u8> {
        self.src.get(self.pos.byte + n).copied()
    }

    /// Consume and return the next byte.
    pub fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos.byte += 1;
        if b == b'\n' {
            self.pos.line += 1;
            self.pos.col = 1;
        } else {
            self.pos.col += 1;
        }
        Some(b)
    }

    /// Slice the underlying source between two byte offsets. Both must
    /// be valid (this is checked by debug_assert).
    pub fn slice(&self, start: usize, end: usize) -> &'src str {
        debug_assert!(end <= self.src.len() && start <= end);
        // Safety: we only ever bump on UTF-8 code unit boundaries because
        // Stem's significant characters are ASCII; multi-byte sequences
        // are stepped through one byte at a time but only complete
        // code points appear in slice boundaries we generate.
        // Defensive: validate at runtime in debug builds.
        let bytes = &self.src[start..end];
        std::str::from_utf8(bytes).expect("slice spans valid UTF-8 boundaries")
    }

    pub fn skip_whitespace_inline(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' {
                self.bump();
            } else {
                break;
            }
        }
    }

    pub fn skip_whitespace_and_newlines(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.bump();
            } else {
                break;
            }
        }
    }

    /// Scan an identifier: `[a-zA-Z][a-zA-Z0-9_-]*`. Returns the slice
    /// and the span; on no match, returns None and leaves cursor in
    /// place.
    pub fn scan_ident(&mut self) -> Option<(&'src str, Span)> {
        let start = self.pos;
        match self.peek()? {
            b'a'..=b'z' | b'A'..=b'Z' => {}
            _ => return None,
        }
        self.bump();
        while let Some(b) = self.peek() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' => {
                    self.bump();
                }
                _ => break,
            }
        }
        let end = self.pos;
        let s = self.slice(start.byte, end.byte);
        Some((s, Span::new(start, end)))
    }

    /// Returns true if the next thing looks like the start of a function
    /// call: an identifier immediately followed by `(` or `[`. Does not
    /// consume. (The `[` form is the `ident[props](args)` variant.)
    pub fn at_function_call(&self) -> bool {
        let mut i = 0;
        match self.peek_at(0) {
            Some(b'a'..=b'z') | Some(b'A'..=b'Z') => i += 1,
            _ => return false,
        }
        while let Some(b) = self.peek_at(i) {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' => i += 1,
                _ => break,
            }
        }
        match self.peek_at(i) {
            Some(b'(') => true,
            // Props-before form: ident `[` ... `]` (...). Only treat as
            // function if there's a `(` somewhere after the closing `]`
            // on the same logical run — we approximate by scanning ahead
            // until newline or `]`. Cheap enough at the call site.
            Some(b'[') => self.has_arg_group_after_brackets(i),
            _ => false,
        }
    }

    fn has_arg_group_after_brackets(&self, start_i: usize) -> bool {
        // Walk past balanced []. If we land on `(`, it's a call. Otherwise no.
        let mut depth: u32 = 0;
        let mut i = start_i;
        while let Some(b) = self.peek_at(i) {
            match b {
                b'[' => depth += 1,
                b']' => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                    if depth == 0 {
                        return self.peek_at(i + 1) == Some(b'(');
                    }
                }
                b'\n' => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }
}
