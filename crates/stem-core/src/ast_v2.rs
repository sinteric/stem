//! AST for Stem v2. See `docs/grammar-v2.md` §11.
//!
//! Coexists with the v1 AST in `ast.rs` during the migration; will
//! become the only AST once v2 parser/renderer/LSP are wired and the
//! playground toggle ships.
//!
//! Design notes:
//! - One uniform `Block` type — no chained args, no `CallKind`.
//! - Body is one of three explicit shapes: none, text, children.
//! - Inline elements live inside `Body::Text` as `TextPiece::Inline`,
//!   carrying a full `Block` (their `@`-prefix is consumed by the parser
//!   and is not stored on the AST — that's a source-syntax detail).
//! - Property values are either bare or quoted; both decode to a
//!   `String`, with `Bare` vs `Quoted` preserved so the formatter can
//!   round-trip the author's choice.

use crate::span::Span;

/// A parsed `.stem` file under the v2 grammar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    pub metadata: Metadata,
    pub blocks: Vec<Block>,
}

/// The `[k:v, k:v]` header. Same shape as v1, retained as-is.
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

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).map(PropertyValue::as_str)
    }
}

/// The core uniform block. Every syntactic element in v2 is a `Block`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: String,
    pub name_span: Span,
    pub properties: Vec<Property>,
    pub body: Body,
    /// Whether the source used the `@`-prefix form. Always false for
    /// top-level/nested-block positions; always true when this block
    /// was parsed in inline position (inside a text body). Renderers
    /// and the formatter need this to round-trip source accurately.
    pub inline_form: bool,
    pub span: Span,
}

impl Block {
    /// Convenience: return the text content if the body is `Text` and
    /// contains only literals (no inline blocks).
    pub fn plain_text(&self) -> Option<String> {
        match &self.body {
            Body::Text(pieces) => {
                let mut s = String::new();
                for p in pieces {
                    match p {
                        TextPiece::Literal { text, .. } => s.push_str(text),
                        TextPiece::Inline(_) => return None,
                    }
                }
                Some(s)
            }
            _ => None,
        }
    }

    /// Convenience: lookup a property by key.
    pub fn prop(&self, key: &str) -> Option<&PropertyValue> {
        self.properties
            .iter()
            .find(|p| p.key == key)
            .map(|p| &p.value)
    }

    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.prop(key).map(PropertyValue::as_str)
    }
}

/// What a block's body looks like in source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// No body. The block is just `name` or `name[props]`.
    None,
    /// A text body `( … )` — either bare-form pieces or a single
    /// quoted-string literal. The parser flattens both into a piece
    /// list so consumers don't branch.
    Text(Vec<TextPiece>),
    /// A block body `{ … }` containing zero or more children blocks.
    Children(Vec<Block>),
}

impl Body {
    pub fn is_none(&self) -> bool {
        matches!(self, Body::None)
    }
    pub fn is_text(&self) -> bool {
        matches!(self, Body::Text(_))
    }
    pub fn is_children(&self) -> bool {
        matches!(self, Body::Children(_))
    }
}

/// One element of a text-body piece list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextPiece {
    Literal { text: String, span: Span },
    /// An inline element. In source this is written as
    /// `@ident[props](text)` etc., and the contained `Block` has
    /// `inline_form: true`.
    Inline(Block),
}

impl TextPiece {
    pub fn span(&self) -> Span {
        match self {
            TextPiece::Literal { span, .. } => *span,
            TextPiece::Inline(b) => b.span,
        }
    }
}

/// A key-value pair in a `[...]` properties block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    pub key: String,
    pub key_span: Span,
    pub value: PropertyValue,
    pub value_span: Span,
}

/// A property's value. The two variants preserve the author's
/// bare/quoted choice so the formatter can round-trip it; both decode
/// to the same `String` payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyValue {
    Bare(String),
    Quoted(String),
}

impl PropertyValue {
    pub fn as_str(&self) -> &str {
        match self {
            PropertyValue::Bare(s) | PropertyValue::Quoted(s) => s,
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

    /// Returns true if the original source used quoted form.
    pub fn was_quoted(&self) -> bool {
        matches!(self, PropertyValue::Quoted(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Pos;

    fn span(line: u32, col: u32) -> Span {
        Span::new(Pos::new(0, line, col), Pos::new(0, line, col))
    }

    #[test]
    fn block_no_body_constructs() {
        let b = Block {
            name: "pagebreak".into(),
            name_span: span(1, 1),
            properties: Vec::new(),
            body: Body::None,
            inline_form: false,
            span: span(1, 1),
        };
        assert!(b.body.is_none());
        assert!(b.plain_text().is_none());
    }

    #[test]
    fn block_text_body_plain_text() {
        let b = Block {
            name: "p".into(),
            name_span: span(1, 1),
            properties: Vec::new(),
            body: Body::Text(vec![TextPiece::Literal {
                text: "hello".into(),
                span: span(1, 3),
            }]),
            inline_form: false,
            span: span(1, 1),
        };
        assert_eq!(b.plain_text().as_deref(), Some("hello"));
    }

    #[test]
    fn block_text_body_with_inline_no_plain_text() {
        let inline_block = Block {
            name: "text".into(),
            name_span: span(1, 10),
            properties: Vec::new(),
            body: Body::Text(vec![TextPiece::Literal {
                text: "red".into(),
                span: span(1, 15),
            }]),
            inline_form: true,
            span: span(1, 10),
        };
        let outer = Block {
            name: "p".into(),
            name_span: span(1, 1),
            properties: Vec::new(),
            body: Body::Text(vec![
                TextPiece::Literal {
                    text: "the ".into(),
                    span: span(1, 3),
                },
                TextPiece::Inline(inline_block),
            ]),
            inline_form: false,
            span: span(1, 1),
        };
        assert!(outer.plain_text().is_none());
    }

    #[test]
    fn property_value_coercions() {
        let v = PropertyValue::Bare("42".into());
        assert_eq!(v.as_str(), "42");
        assert_eq!(v.as_i64(), Some(42));
        assert_eq!(v.as_bool(), None);

        let v = PropertyValue::Quoted("yes".into());
        assert_eq!(v.as_bool(), Some(true));
        assert!(v.was_quoted());
    }
}
