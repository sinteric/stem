//! Conversions between Stem's `Span`/`Pos` and LSP's `Range`/`Position`.
//!
//! LSP positions are 0-based; Stem positions are 1-based with byte columns.
//! For a real LSP we should compute UTF-16 code unit columns, but byte
//! columns work for ASCII (the common case in source files) and we
//! convert utf-8 byte columns -> utf-16 with the document text when
//! precision matters.

use stem_core::span::{Pos, Span};
use tower_lsp::lsp_types::{Position, Range};

pub fn pos_to_lsp(p: Pos) -> Position {
    Position {
        line: p.line.saturating_sub(1),
        character: p.col.saturating_sub(1),
    }
}

pub fn span_to_range(s: Span) -> Range {
    Range {
        start: pos_to_lsp(s.start),
        end: pos_to_lsp(s.end),
    }
}

pub fn severity_to_lsp(s: stem_core::diagnostic::Severity) -> tower_lsp::lsp_types::DiagnosticSeverity {
    use tower_lsp::lsp_types::DiagnosticSeverity as L;
    use stem_core::diagnostic::Severity as S;
    match s {
        S::Error => L::ERROR,
        S::Warning => L::WARNING,
        S::Hint => L::HINT,
    }
}
