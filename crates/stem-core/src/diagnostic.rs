//! Diagnostic type shared by parser, validator, and renderers.

use crate::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// A short machine-readable code, e.g. "parse.unclosed_paren".
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            span,
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            span,
        }
    }

    pub fn hint(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Hint,
            code,
            message: message.into(),
            span,
        }
    }
}
