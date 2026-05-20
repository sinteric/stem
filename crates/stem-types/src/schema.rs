//! Function and property schemas keyed by document type.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocumentType {
    Document,
    Presentation,
    Sheet,
}

impl DocumentType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "document" => Some(Self::Document),
            "presentation" => Some(Self::Presentation),
            "sheet" => Some(Self::Sheet),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Presentation => "presentation",
            Self::Sheet => "sheet",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionSchema {
    pub name: &'static str,
    /// Which document types this function is valid in. Empty = all.
    pub allowed_in: &'static [DocumentType],
    /// How many argument groups (`(...)`) the function expects.
    pub arity: ArgArity,
    /// Per-argument-group hint shown in LSP completion / docs.
    pub arg_hints: &'static [&'static str],
    pub properties: &'static [PropertySchema],
    /// One-line summary for LSP hover.
    pub doc: &'static str,
    /// Whether the function is typically used at block level (used to
    /// emit a hint, never an error, if the parser classifies it
    /// differently).
    pub block_preferred: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArgArity {
    /// Exactly N argument groups.
    Exact(u8),
    /// At least N, at most M.
    Range(u8, u8),
    /// Any number, including zero.
    Any,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertySchema {
    pub name: &'static str,
    pub kind: ValueKind,
    pub doc: &'static str,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueKind {
    /// Any string.
    String,
    /// Signed integer.
    Integer,
    /// Boolean (`true`/`false`/`yes`/`no`).
    Bool,
    /// One of a fixed set.
    Enum(&'static [&'static str]),
    /// Either a registered theme color name or a `#rrggbb` literal.
    Color,
}

/// Function registry keyed by name. We keep all functions for all types
/// in one map and let each function declare which types it's valid in —
/// this is far less duplication than per-type maps because most
/// functions are document-only or shared across document/presentation.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    by_name: BTreeMap<&'static str, FunctionSchema>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, schema: FunctionSchema) -> &mut Self {
        self.by_name.insert(schema.name, schema);
        self
    }

    pub fn get(&self, name: &str) -> Option<&FunctionSchema> {
        self.by_name.get(name)
    }

    pub fn names_for(&self, doc_type: DocumentType) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self
            .by_name
            .values()
            .filter(|s| s.allowed_in.is_empty() || s.allowed_in.contains(&doc_type))
            .map(|s| s.name)
            .collect();
        out.sort();
        out
    }
}

/// Build the registry of bundled functions.
pub fn default_registry() -> Registry {
    let mut r = Registry::new();
    for schema in BUILTIN_FUNCTIONS.iter().cloned() {
        r.insert(schema);
    }
    r
}

// ============================================================
// Built-in schemas
// ============================================================

use ArgArity::*;
use DocumentType::{Document as Doc, Presentation as Pres, Sheet};

const SECTION: FunctionSchema = FunctionSchema {
    name: "section",
    allowed_in: &[Doc],
    arity: Range(1, 2),
    arg_hints: &["id", "body"],
    properties: &[
        PropertySchema {
            name: "id",
            kind: ValueKind::String,
            doc: "Section identifier",
            required: false,
        },
        PropertySchema {
            name: "title",
            kind: ValueKind::String,
            doc: "Display title (overrides the H1 inside the body)",
            required: false,
        },
    ],
    doc: "A top-level section of a document. `section(cover)(...)` chains an id with a body.",
    block_preferred: true,
};

const LAYOUT: FunctionSchema = FunctionSchema {
    name: "layout",
    allowed_in: &[Doc, Pres],
    arity: Exact(2),
    arg_hints: &["kind", "body"],
    properties: &[],
    doc: "A multi-column layout container. `kind` is one of: two-column, three-column, sidebar.",
    block_preferred: true,
};

const COL: FunctionSchema = FunctionSchema {
    name: "col",
    allowed_in: &[Doc, Pres],
    arity: Exact(1),
    arg_hints: &["content"],
    properties: &[PropertySchema {
        name: "width",
        kind: ValueKind::String,
        doc: "Optional fractional width hint, e.g. `1`, `2`, `auto`",
        required: false,
    }],
    doc: "One column inside a `layout`.",
    block_preferred: true,
};

const FOOTNOTE: FunctionSchema = FunctionSchema {
    name: "footnote",
    allowed_in: &[Doc, Pres],
    arity: Exact(1),
    arg_hints: &["text"],
    properties: &[],
    doc: "An inline footnote reference.",
    block_preferred: false,
};

const NOTE: FunctionSchema = FunctionSchema {
    name: "note",
    allowed_in: &[Doc, Pres],
    arity: Exact(1),
    arg_hints: &["text"],
    properties: &[],
    doc: "A non-printing annotation. Speaker notes in presentation mode.",
    block_preferred: false,
};

const DATE: FunctionSchema = FunctionSchema {
    name: "date",
    allowed_in: &[],
    arity: Exact(1),
    arg_hints: &["text"],
    properties: &[],
    doc: "Render a date span — content is the raw date string.",
    block_preferred: false,
};

const TOC: FunctionSchema = FunctionSchema {
    name: "toc",
    allowed_in: &[Doc],
    arity: ArgArity::Any,
    arg_hints: &[],
    properties: &[PropertySchema {
        name: "depth",
        kind: ValueKind::Integer,
        doc: "Maximum heading level to include (default: 3)",
        required: false,
    }],
    doc: "Table of contents marker.",
    block_preferred: true,
};

const PAGEBREAK: FunctionSchema = FunctionSchema {
    name: "pagebreak",
    allowed_in: &[Doc],
    arity: ArgArity::Any,
    arg_hints: &[],
    properties: &[],
    doc: "Force a page break in paged output.",
    block_preferred: true,
};

const TEXT: FunctionSchema = FunctionSchema {
    name: "text",
    allowed_in: &[],
    arity: Exact(1),
    arg_hints: &["content"],
    properties: &[
        PropertySchema {
            name: "color",
            kind: ValueKind::Color,
            doc: "Foreground color, by theme name or `#rrggbb`",
            required: false,
        },
        PropertySchema {
            name: "bg",
            kind: ValueKind::Color,
            doc: "Background color",
            required: false,
        },
        PropertySchema {
            name: "weight",
            kind: ValueKind::Enum(&["light", "regular", "bold"]),
            doc: "Font weight",
            required: false,
        },
    ],
    doc: "Inline styled text.",
    block_preferred: false,
};

const TABLE: FunctionSchema = FunctionSchema {
    name: "table",
    allowed_in: &[Doc, Sheet],
    arity: Exact(1),
    arg_hints: &["rows"],
    properties: &[PropertySchema {
        name: "border",
        kind: ValueKind::Enum(&["none", "outer", "all"]),
        doc: "Border policy",
        required: false,
    }],
    doc: "A table. Body is a sequence of `row(...)` calls.",
    block_preferred: true,
};

const ROW: FunctionSchema = FunctionSchema {
    name: "row",
    allowed_in: &[Doc, Sheet],
    arity: Range(1, 2),
    arg_hints: &["kind", "cells"],
    properties: &[],
    doc: "A table row. `row(header)(...)` marks a header row.",
    block_preferred: true,
};

const CELL: FunctionSchema = FunctionSchema {
    name: "cell",
    allowed_in: &[Doc, Sheet],
    arity: Exact(1),
    arg_hints: &["content"],
    properties: &[
        PropertySchema {
            name: "span",
            kind: ValueKind::Integer,
            doc: "Column span",
            required: false,
        },
        PropertySchema {
            name: "rowspan",
            kind: ValueKind::Integer,
            doc: "Row span",
            required: false,
        },
        PropertySchema {
            name: "bg",
            kind: ValueKind::Color,
            doc: "Cell background color",
            required: false,
        },
        PropertySchema {
            name: "align",
            kind: ValueKind::Enum(&["left", "center", "right"]),
            doc: "Horizontal alignment",
            required: false,
        },
    ],
    doc: "A table cell.",
    block_preferred: false,
};

const SLIDE: FunctionSchema = FunctionSchema {
    name: "slide",
    allowed_in: &[Pres],
    arity: Range(1, 2),
    arg_hints: &["id", "body"],
    properties: &[PropertySchema {
        name: "layout",
        kind: ValueKind::String,
        doc: "Slide layout name from the theme",
        required: false,
    }],
    doc: "A single slide in a presentation.",
    block_preferred: true,
};

const SPEAKER_NOTE: FunctionSchema = FunctionSchema {
    name: "speaker-note",
    allowed_in: &[Pres],
    arity: Exact(1),
    arg_hints: &["text"],
    properties: &[],
    doc: "Speaker notes attached to the surrounding slide.",
    block_preferred: true,
};

const TRANSITION: FunctionSchema = FunctionSchema {
    name: "transition",
    allowed_in: &[Pres],
    arity: Exact(1),
    arg_hints: &["kind"],
    properties: &[],
    doc: "Transition between slides (fade, slide-left, none).",
    block_preferred: false,
};

const CHART: FunctionSchema = FunctionSchema {
    name: "chart",
    allowed_in: &[Doc, Pres, Sheet],
    arity: Exact(1),
    arg_hints: &["data"],
    properties: &[PropertySchema {
        name: "type",
        kind: ValueKind::Enum(&["bar", "line", "pie", "scatter"]),
        doc: "Chart type",
        required: true,
    }],
    doc: "A chart rendered from inline data.",
    block_preferred: true,
};

const FORMULA: FunctionSchema = FunctionSchema {
    name: "formula",
    allowed_in: &[Sheet],
    arity: Exact(1),
    arg_hints: &["expr"],
    properties: &[],
    doc: "A spreadsheet formula expression.",
    block_preferred: false,
};

const BUILTIN_FUNCTIONS: &[FunctionSchema] = &[
    SECTION,
    LAYOUT,
    COL,
    FOOTNOTE,
    NOTE,
    DATE,
    TOC,
    PAGEBREAK,
    TEXT,
    TABLE,
    ROW,
    CELL,
    SLIDE,
    SPEAKER_NOTE,
    TRANSITION,
    CHART,
    FORMULA,
];
