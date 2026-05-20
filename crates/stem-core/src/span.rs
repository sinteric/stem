//! Source spans. We store both byte offsets (for slicing) and
//! 1-based line/column pairs (for human-facing diagnostics and LSP).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Pos {
    /// Byte offset from start of file.
    pub byte: usize,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number, counted in UTF-8 byte offsets within the line.
    pub col: u32,
}

impl Pos {
    pub const fn new(byte: usize, line: u32, col: u32) -> Self {
        Self { byte, line, col }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: Pos,
    pub end: Pos,
}

impl Span {
    pub const fn new(start: Pos, end: Pos) -> Self {
        Self { start, end }
    }

    /// Span that covers the single-character position `p`.
    pub fn point(p: Pos) -> Self {
        Self { start: p, end: p }
    }

    /// Smallest span that covers both `self` and `other`.
    pub fn merge(self, other: Span) -> Span {
        let start = if self.start.byte <= other.start.byte {
            self.start
        } else {
            other.start
        };
        let end = if self.end.byte >= other.end.byte {
            self.end
        } else {
            other.end
        };
        Span { start, end }
    }
}
