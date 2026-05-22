//! Markdown → Stem AST.
//!
//! MVP scope: headings (`#` through `######`), paragraphs, ordered and
//! unordered lists, fenced code blocks, inline code, links, emphasis
//! (`*…*` and `_…_`), strong (`**…**` and `__…__`).
//!
//! Out of scope for v0.1: HTML passthrough, footnotes, tables, math,
//! task lists, autolinks, GFM extensions. Add as users surface needs.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use stem_core::ast::{Block, Body, Document, Metadata, Property, PropertyValue, TextPiece};
use stem_core::span::Span;
use stem_core::Importer;
use thiserror::Error;

/// Importer for CommonMark (with a small enabled set of extensions).
#[derive(Default)]
pub struct MarkdownImporter;

impl MarkdownImporter {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Error)]
pub enum MarkdownError {
    // Future: surface lossy-mapping warnings here. For v0.1 there's
    // nothing the parser itself can fail on — pulldown-cmark accepts
    // any input.
    #[error("markdown import error: {0}")]
    Other(String),
}

impl Importer for MarkdownImporter {
    type Input = &'static str;
    // Note: this Input type is `&'static str` only because the trait
    // requires a single associated type. In practice consumers call
    // `import_str` below, which takes any `&str` and a `Document`
    // owned-by-the-call.
    type Error = MarkdownError;

    fn import(&self, input: Self::Input) -> Result<Document, Self::Error> {
        Ok(import_str(input))
    }
}

/// Import a Markdown string into a Stem [`Document`].
///
/// This is the practical entry point. The [`Importer`] trait impl above
/// is awkward because the trait's `Input` is a single type — for
/// markdown specifically we want any `&str`, so prefer this function.
pub fn import_str(src: &str) -> Document {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(src, opts);

    let mut blocks: Vec<Block> = Vec::new();
    let mut stack: Vec<Builder> = vec![Builder::Root];

    for event in parser {
        match event {
            Event::Start(tag) => start_tag(&mut stack, tag),
            Event::End(tag) => end_tag(&mut stack, &mut blocks, tag),
            Event::Text(t) => push_text(&mut stack, t.into_string()),
            Event::Code(t) => push_inline(&mut stack, inline_code(t.into_string())),
            Event::SoftBreak | Event::HardBreak => push_text(&mut stack, " ".into()),
            Event::Html(_) | Event::InlineHtml(_) => {
                // HTML passthrough is out of MVP scope; ignore.
            }
            Event::FootnoteReference(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {
                // Out of MVP scope.
            }
            Event::Rule => blocks.push(named_block("hr")),
            Event::TaskListMarker(_) => {}
        }
    }

    Document {
        metadata: Metadata {
            properties: vec![Property {
                key: "type".to_string(),
                key_span: Span::default(),
                value: PropertyValue::Bare("document".to_string()),
                value_span: Span::default(),
            }],
            span: Span::default(),
        },
        blocks,
    }
}

// --- internal builders --------------------------------------------------

/// One frame of the block/inline-construction stack.
enum Builder {
    /// Top-level container — finished blocks flow past this frame into
    /// the document's block list.
    Root,
    /// A block that will receive text-body pieces (paragraph, heading,
    /// list-item with inline content, blockquote).
    TextBlock {
        name: String,
        properties: Vec<Property>,
        pieces: Vec<TextPiece>,
    },
    /// A block that will receive child blocks (lists).
    ChildBlock {
        name: String,
        properties: Vec<Property>,
        children: Vec<Block>,
    },
}

fn start_tag(stack: &mut Vec<Builder>, tag: Tag) {
    match tag {
        Tag::Heading { level, .. } => {
            stack.push(Builder::TextBlock {
                name: heading_name(level).to_string(),
                properties: vec![],
                pieces: vec![],
            });
        }
        Tag::Paragraph => stack.push(Builder::TextBlock {
            name: "p".into(),
            properties: vec![],
            pieces: vec![],
        }),
        Tag::BlockQuote(_) => stack.push(Builder::TextBlock {
            name: "blockquote".into(),
            properties: vec![],
            pieces: vec![],
        }),
        Tag::List(start_opt) => {
            let name = if start_opt.is_some() { "ol" } else { "ul" };
            let mut props = vec![];
            if let Some(start) = start_opt {
                if start != 1 {
                    props.push(string_prop("start", &start.to_string()));
                }
            }
            stack.push(Builder::ChildBlock {
                name: name.into(),
                properties: props,
                children: vec![],
            });
        }
        Tag::Item => stack.push(Builder::TextBlock {
            name: "li".into(),
            properties: vec![],
            pieces: vec![],
        }),
        Tag::CodeBlock(kind) => {
            let mut props = vec![];
            if let CodeBlockKind::Fenced(lang) = kind {
                let lang = lang.trim();
                if !lang.is_empty() {
                    props.push(string_prop("lang", lang));
                }
            }
            stack.push(Builder::TextBlock {
                name: "code".into(),
                properties: props,
                pieces: vec![],
            });
        }
        Tag::Emphasis | Tag::Strong | Tag::Strikethrough => {
            // Wrap the inline run in a @text element with style props.
            let prop = match tag {
                Tag::Emphasis => string_prop("style", "italic"),
                Tag::Strong => string_prop("weight", "bold"),
                _ => string_prop("decoration", "strike"),
            };
            stack.push(Builder::TextBlock {
                name: "text".into(),
                properties: vec![prop],
                pieces: vec![],
            });
        }
        Tag::Link { dest_url, title, .. } => {
            let mut props = vec![string_prop("to", dest_url.as_ref())];
            if !title.is_empty() {
                props.push(string_prop("title", title.as_ref()));
            }
            stack.push(Builder::TextBlock {
                name: "link".into(),
                properties: props,
                pieces: vec![],
            });
        }
        // Unhandled: Image, Table*, FootnoteDefinition, HtmlBlock,
        // MetadataBlock, DefinitionList*. MVP scope.
        _ => stack.push(Builder::TextBlock {
            name: "p".into(), // fallback container so we don't lose content
            properties: vec![],
            pieces: vec![],
        }),
    }
}

fn end_tag(stack: &mut Vec<Builder>, root_blocks: &mut Vec<Block>, _tag: TagEnd) {
    let frame = stack.pop().expect("balanced events");
    // An inline-named element (text, link, code) becomes a TextPiece::Inline
    // in the parent's body — flag inline_form accordingly. Everything else
    // is a block at top-level or nested in a list, inline_form: false.
    let finished = match frame {
        Builder::TextBlock { name, properties, pieces } => {
            let inline_form = is_inline_name(&name);
            Block {
                name: name.clone(),
                name_span: Span::default(),
                properties,
                body: if pieces.is_empty() { Body::None } else { Body::Text(pieces) },
                inline_form,
                span: Span::default(),
            }
        }
        Builder::ChildBlock { name, properties, children } => Block {
            name: name.clone(),
            name_span: Span::default(),
            properties,
            body: if children.is_empty() { Body::None } else { Body::Children(children) },
            inline_form: false,
            span: Span::default(),
        },
        Builder::Root => return, // never popped this way
    };
    push_block(stack, root_blocks, finished);
}

fn is_inline_name(name: &str) -> bool {
    matches!(name, "text" | "link" | "code-inline")
}

fn push_block(stack: &mut Vec<Builder>, root_blocks: &mut Vec<Block>, block: Block) {
    match stack.last_mut() {
        Some(Builder::ChildBlock { children, .. }) => children.push(block),
        Some(Builder::TextBlock { pieces, .. }) => {
            pieces.push(TextPiece::Inline(block));
        }
        Some(Builder::Root) | None => root_blocks.push(block),
    }
}

fn push_text(stack: &mut Vec<Builder>, text: String) {
    if let Some(Builder::TextBlock { pieces, .. }) = stack.last_mut() {
        pieces.push(TextPiece::Literal {
            text,
            span: Span::default(),
        });
    }
}

fn push_inline(stack: &mut Vec<Builder>, block: Block) {
    if let Some(Builder::TextBlock { pieces, .. }) = stack.last_mut() {
        pieces.push(TextPiece::Inline(block));
    }
}

// --- helpers ------------------------------------------------------------

fn heading_name(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "h1",
        HeadingLevel::H2 => "h2",
        HeadingLevel::H3 => "h3",
        HeadingLevel::H4 => "h4",
        HeadingLevel::H5 => "h5",
        HeadingLevel::H6 => "h6",
    }
}

fn string_prop(key: &str, value: &str) -> Property {
    Property {
        key: key.to_string(),
        key_span: Span::default(),
        value: PropertyValue::Bare(value.to_string()),
        value_span: Span::default(),
    }
}

fn inline_code(text: String) -> Block {
    Block {
        name: "code".to_string(),
        name_span: Span::default(),
        properties: vec![],
        body: Body::Text(vec![TextPiece::Literal {
            text,
            span: Span::default(),
        }]),
        inline_form: true,
        span: Span::default(),
    }
}

fn named_block(name: &str) -> Block {
    Block {
        name: name.to_string(),
        name_span: Span::default(),
        properties: vec![],
        body: Body::None,
        inline_form: false,
        span: Span::default(),
    }
}
