//! Stem AST, diagnostics, and theme types.
//!
//! This crate is intentionally dependency-light: the parser, validator,
//! LSP, and renderers all depend on it, so it sits at the bottom of the
//! workspace and never depends on any of them.

pub mod ast;
pub mod diagnostic;
pub mod span;
pub mod theme;

pub use diagnostic::{Diagnostic, Severity};
pub use span::{Pos, Span};
pub use theme::{Theme, ThemeColor};
