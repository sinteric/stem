//! AST for Stem documents.
//!
//! Two layers:
//! - Raw layer: `Node`, `FunctionCall`, `Content`, `Property` — what the
//!   parser produces directly. Faithful to source.
//! - Cooked layer: `Block`, `Inline`, `Paragraph`, `ListItem` — the result
//!   of running the markdown-flavored second pass on raw content runs.
//!
//! Renderers consume the cooked layer. The validator and LSP work on the
//! raw layer (so unknown functions still appear).

use crate::span::Span;

/// A parsed `.stem` file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    pub metadata: Metadata,
    pub nodes: Vec<Node>,
}

/// The `[type:..., ...]` header. Missing fields fall back to defaults at
/// the validation layer, not here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub span: Span,
    pub properties: Vec<Property>,
}

impl Metadata {
    pub fn get(&self, key: &str) -> Option<&PropertyValue> {
        self.properties
            .iter()
            .find(|p| p.key == key)
            .map(|p| &p.value)
    }

    pub fn get_str<'a>(&'a self, key: &str) -> Option<&'a str> {
        self.get(key).map(PropertyValue::as_str)
    }
}

/// A property key/value pair in a `[...]` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    pub key: String,
    pub key_span: Span,
    pub value: PropertyValue,
    pub value_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyValue {
    /// A bare token like `red`, `2`, or `corporate`. Type-coerced at the
    /// validator depending on the function's schema.
    Bare(String),
    /// A `"quoted string"` literal.
    String(String),
}

impl PropertyValue {
    pub fn as_str(&self) -> &str {
        match self {
            PropertyValue::Bare(s) | PropertyValue::String(s) => s,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.as_str().parse().ok()
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.as_str() {
            "true" | "yes" | "on" => Some(true),
            "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }
}

/// A top-level node — either a function call or a stretch of raw text
/// (which the second pass cooks into paragraphs, headings, lists).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Call(FunctionCall),
    Text(TextRun),
}

impl Node {
    pub fn span(&self) -> Span {
        match self {
            Node::Call(c) => c.span,
            Node::Text(t) => t.span,
        }
    }
}

/// A `name(arg)(arg)...[props]` call, as parsed.
///
/// Each `(...)` is one *argument group*. Most calls have a single group
/// (`cell(value)`); some take two (`section(cover)(body...)`) where the
/// first acts as a tag/id and the last is the body. The validator and
/// renderer decide the per-function meaning. Use `body()` for the
/// renderer-friendly view.
///
/// `kind` is `Block` if *any* argument group contains a literal newline
/// at depth zero (outside nested calls), otherwise `Inline`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionCall {
    pub name: String,
    pub name_span: Span,
    pub kind: CallKind,
    pub args: Vec<Vec<Content>>,
    pub properties: Vec<Property>,
    pub span: Span,
}

impl FunctionCall {
    /// The most relevant content for a renderer: the last argument
    /// group, which is the body for chained calls and the only content
    /// for single-group calls. Empty slice if there are no arg groups.
    pub fn body(&self) -> &[Content] {
        self.args.last().map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The leading "header" arg, present only on chained calls like
    /// `section(cover)(body)`. None for single-group calls.
    pub fn header(&self) -> Option<&[Content]> {
        if self.args.len() <= 1 {
            None
        } else {
            self.args.first().map(|v| v.as_slice())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CallKind {
    Block,
    Inline,
}

/// One element of a function's content list: either a literal text run
/// or a nested call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    Text(TextRun),
    Call(FunctionCall),
}

impl Content {
    pub fn span(&self) -> Span {
        match self {
            Content::Text(t) => t.span,
            Content::Call(c) => c.span,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRun {
    pub text: String,
    pub span: Span,
}

// =============== Cooked layer (markdown second pass) ===============
//
// The cooked layer is constructed lazily by `stem_parser::cook` (or by
// renderers as needed). It is *not* what the parser emits.

/// A block-level cooked element produced by the markdown pass over a
/// stretch of raw text. Lives alongside `Content::Call` in the cooked
/// content list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        runs: Vec<Inline>,
        span: Span,
    },
    Paragraph(Paragraph),
    List {
        kind: ListKind,
        items: Vec<ListItem>,
        span: Span,
    },
    /// A function call that the second pass surfaced as a block (because
    /// its raw form is a `Block`-kind call).
    Call(FunctionCall),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paragraph {
    pub runs: Vec<Inline>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListKind {
    Unordered,
    Ordered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub runs: Vec<Inline>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text {
        text: String,
        style: TextStyle,
        span: Span,
    },
    Call(FunctionCall),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownLine {
    pub kind: LineKind,
    pub raw: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineKind {
    Blank,
    Heading(u8),
    Bullet,
    Ordered,
    Paragraph,
}
