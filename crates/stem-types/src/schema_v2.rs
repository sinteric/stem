//! Grammar v2 schema registry. Mirrors `docs/schema.md`.
//!
//! Hand-keyed for now. Once `docs/schema.md`'s `stem-schema` blocks
//! can be machine-extracted, this module will load them at startup
//! instead — design is identical, just the source of truth moves.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    BlockContainer,
    BlockLeaf,
    BlockMarker,
    Inline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyKind {
    None,
    Text,
    Children,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueKind {
    String,
    Integer,
    Bool,
    Color,
    Length,
    Address,
    Style,
    Enum(&'static [&'static str]),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyDef {
    pub name: &'static str,
    pub kind: ValueKind,
    pub required: bool,
    pub doc: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementSchema {
    pub name: &'static str,
    pub categories: &'static [Category],
    /// Empty means "all doc types"
    pub doc_types: &'static [DocumentType],
    pub bodies: &'static [BodyKind],
    pub parents: &'static [&'static str],
    pub children: &'static [&'static str],
    pub properties: &'static [PropertyDef],
    pub doc: &'static str,
}

/// Registry of element schemas. Supports multiple schemas per element
/// name (e.g. `col` exists both as a layout-column for document/presentation
/// and as a sheet-column for sheet). Lookup is by (name, doc_type) and
/// picks the matching variant, with a universal-fallback for elements
/// that declare empty `doc_types`.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    by_name: BTreeMap<&'static str, Vec<ElementSchema>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, schema: ElementSchema) -> &mut Self {
        self.by_name.entry(schema.name).or_default().push(schema);
        self
    }

    /// Look up a schema for `name` valid in `doc_type`. Picks the
    /// doc-type-specific variant when present; falls back to a
    /// universal variant (one with `doc_types: &[]`).
    pub fn get(&self, name: &str, doc_type: DocumentType) -> Option<&ElementSchema> {
        let variants = self.by_name.get(name)?;
        // Prefer an exact doc_type match
        if let Some(s) = variants
            .iter()
            .find(|s| s.doc_types.contains(&doc_type))
        {
            return Some(s);
        }
        // Then a universal (empty doc_types) variant
        variants.iter().find(|s| s.doc_types.is_empty())
    }

    /// Returns true if any schema for `name` exists (in any doc type).
    pub fn has_any(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    pub fn names_for(&self, doc_type: DocumentType) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self
            .by_name
            .iter()
            .filter(|(_, variants)| {
                variants
                    .iter()
                    .any(|s| s.doc_types.is_empty() || s.doc_types.contains(&doc_type))
            })
            .map(|(name, _)| *name)
            .collect();
        out.sort();
        out
    }
}

pub fn default_registry() -> Registry {
    let mut r = Registry::new();
    for s in BUILTINS.iter().cloned() {
        r.insert(s);
    }
    r
}

// ============================================================
// Builtins — mirrors docs/schema.md (subset for v2.0)
// ============================================================

use BodyKind::{Children, None as NoBody, Text};
use Category::{BlockContainer, BlockLeaf, BlockMarker, Inline};
use DocumentType::{Document as Doc, Presentation as Pres, Sheet};

const ALL: &[DocumentType] = &[];

const HEADING_PROPS: &[PropertyDef] = &[
    PropertyDef {
        name: "id",
        kind: ValueKind::String,
        required: false,
        doc: "Heading id for cross-refs; auto-derived from text if absent",
    },
    PropertyDef {
        name: "numbered",
        kind: ValueKind::Bool,
        required: false,
        doc: "Include in the auto-numbering scheme",
    },
];

// --- Universal inline ---

const TEXT: ElementSchema = ElementSchema {
    name: "text",
    categories: &[Inline, BlockLeaf],
    doc_types: ALL,
    bodies: &[Text],
    parents: &["any-text-body", "any-block-container"],
    children: &[],
    properties: &[
        PropertyDef { name: "color", kind: ValueKind::Color, required: false, doc: "Foreground color" },
        PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
        PropertyDef {
            name: "weight",
            kind: ValueKind::Enum(&["light", "regular", "bold"]),
            required: false,
            doc: "Font weight",
        },
        PropertyDef {
            name: "style",
            kind: ValueKind::Enum(&["italic", "oblique", "normal"]),
            required: false,
            doc: "Font slant",
        },
        PropertyDef {
            name: "decoration",
            kind: ValueKind::Enum(&["none", "underline", "strike"]),
            required: false,
            doc: "Text decoration",
        },
    ],
    doc: "Styled inline text span",
};

const FOOTNOTE: ElementSchema = ElementSchema {
    name: "footnote",
    categories: &[Inline],
    doc_types: ALL,
    bodies: &[Text],
    parents: &["any-text-body"],
    children: &[],
    properties: &[PropertyDef {
        name: "id",
        kind: ValueKind::String,
        required: false,
        doc: "Stable footnote id for cross-references",
    }],
    doc: "Inline footnote reference",
};

const CODE_EL: ElementSchema = ElementSchema {
    name: "code",
    categories: &[Inline, BlockLeaf],
    doc_types: ALL,
    bodies: &[Text],
    parents: &["any-text-body", "any-block-container"],
    children: &[],
    properties: &[
        PropertyDef { name: "lang", kind: ValueKind::String, required: false, doc: "Source language" },
        PropertyDef { name: "numbered", kind: ValueKind::Bool, required: false, doc: "Show line numbers" },
    ],
    doc: "Inline or block monospace code",
};

const LINK: ElementSchema = ElementSchema {
    name: "link",
    categories: &[Inline],
    doc_types: ALL,
    bodies: &[Text],
    parents: &["any-text-body"],
    children: &[],
    properties: &[
        PropertyDef { name: "to", kind: ValueKind::String, required: true, doc: "Target URL or cross-ref" },
        PropertyDef { name: "title", kind: ValueKind::String, required: false, doc: "Tooltip text" },
    ],
    doc: "Hyperlink",
};

const DATE: ElementSchema = ElementSchema {
    name: "date",
    categories: &[Inline, BlockLeaf],
    doc_types: ALL,
    bodies: &[Text],
    parents: &["any-text-body", "any-block-container"],
    children: &[],
    properties: &[PropertyDef {
        name: "format",
        kind: ValueKind::String,
        required: false,
        doc: "Display format hint",
    }],
    doc: "A semantic date span",
};

const MENTION: ElementSchema = ElementSchema {
    name: "mention",
    categories: &[Inline],
    doc_types: ALL,
    bodies: &[Text],
    parents: &["any-text-body"],
    children: &[],
    properties: &[PropertyDef {
        name: "handle",
        kind: ValueKind::String,
        required: false,
        doc: "Backing handle/identifier",
    }],
    doc: "Reference to a person, team, or entity",
};

const MATH: ElementSchema = ElementSchema {
    name: "math",
    categories: &[Inline, BlockLeaf],
    doc_types: ALL,
    bodies: &[Text],
    parents: &["any-text-body", "any-block-container"],
    children: &[],
    properties: &[PropertyDef {
        name: "display",
        kind: ValueKind::Enum(&["inline", "block"]),
        required: false,
        doc: "Render style",
    }],
    doc: "Inline or block math expression",
};

// --- Document structural ---

const SECTION: ElementSchema = ElementSchema {
    name: "section",
    categories: &[BlockContainer],
    doc_types: &[Doc],
    bodies: &[Children, NoBody],
    parents: &["root", "section"],
    children: &["any-block"],
    properties: &[
        PropertyDef { name: "id", kind: ValueKind::String, required: false, doc: "Section identifier" },
        PropertyDef { name: "title", kind: ValueKind::String, required: false, doc: "Display title override" },
    ],
    doc: "Top-level structural division of a document",
};

const LAYOUT: ElementSchema = ElementSchema {
    name: "layout",
    categories: &[BlockContainer],
    doc_types: &[Doc, Pres],
    bodies: &[Children],
    parents: &["any-block-container"],
    children: &["col"],
    properties: &[
        PropertyDef {
            name: "kind",
            kind: ValueKind::Enum(&["two-column", "three-column", "sidebar"]),
            required: true,
            doc: "Layout variant",
        },
        PropertyDef { name: "gap", kind: ValueKind::Length, required: false, doc: "Inter-column gap" },
    ],
    doc: "Multi-column layout container",
};

const COL_LAYOUT: ElementSchema = ElementSchema {
    name: "col",
    categories: &[BlockContainer],
    doc_types: &[Doc, Pres],
    bodies: &[Children],
    parents: &["layout"],
    children: &["any-block"],
    properties: &[PropertyDef {
        name: "width",
        kind: ValueKind::String,
        required: false,
        doc: "Width hint",
    }],
    doc: "One column inside a layout",
};

const PAGEBREAK: ElementSchema = ElementSchema {
    name: "pagebreak",
    categories: &[BlockMarker],
    doc_types: &[Doc],
    bodies: &[NoBody],
    parents: &["any-block-container"],
    children: &[],
    properties: &[],
    doc: "Force a page break in paged output",
};

const HR: ElementSchema = ElementSchema {
    name: "hr",
    categories: &[BlockMarker],
    doc_types: &[Doc],
    bodies: &[NoBody],
    parents: &["any-block-container"],
    children: &[],
    properties: &[],
    doc: "Horizontal rule",
};

// --- Headings h1..h6 ---

macro_rules! heading_schema {
    ($name:ident, $literal:literal) => {
        const $name: ElementSchema = ElementSchema {
            name: $literal,
            categories: &[BlockLeaf],
            doc_types: &[Doc],
            bodies: &[Text],
            parents: &["root", "any-block-container"],
            children: &[],
            properties: HEADING_PROPS,
            doc: "Heading",
        };
    };
}
heading_schema!(H1, "h1");
heading_schema!(H2, "h2");
heading_schema!(H3, "h3");
heading_schema!(H4, "h4");
heading_schema!(H5, "h5");
heading_schema!(H6, "h6");

// --- Document block content ---

const P: ElementSchema = ElementSchema {
    name: "p",
    categories: &[BlockLeaf],
    doc_types: &[Doc, Pres],
    bodies: &[Text],
    parents: &["any-block-container"],
    children: &[],
    properties: &[PropertyDef {
        name: "align",
        kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
        required: false,
        doc: "Horizontal alignment",
    }],
    doc: "Paragraph",
};

const NOTE: ElementSchema = ElementSchema {
    name: "note",
    categories: &[BlockLeaf],
    doc_types: &[Doc, Pres],
    bodies: &[Text],
    parents: &["any-block-container"],
    children: &[],
    properties: &[PropertyDef {
        name: "kind",
        kind: ValueKind::Enum(&["info", "warning", "tip", "caution"]),
        required: false,
        doc: "Visual variant",
    }],
    doc: "Callout note",
};

const BLOCKQUOTE: ElementSchema = ElementSchema {
    name: "blockquote",
    categories: &[BlockLeaf],
    doc_types: &[Doc, Pres],
    bodies: &[Text],
    parents: &["any-block-container"],
    children: &[],
    properties: &[PropertyDef {
        name: "cite",
        kind: ValueKind::String,
        required: false,
        doc: "Source citation",
    }],
    doc: "Multi-line quotation block",
};

const IMAGE: ElementSchema = ElementSchema {
    name: "image",
    categories: &[BlockMarker],
    doc_types: &[Doc, Pres],
    bodies: &[NoBody],
    parents: &["any-block-container"],
    children: &[],
    properties: &[
        PropertyDef { name: "src", kind: ValueKind::String, required: true, doc: "Image path or URL" },
        PropertyDef { name: "alt", kind: ValueKind::String, required: true, doc: "Alt text for accessibility" },
        PropertyDef { name: "w", kind: ValueKind::Length, required: false, doc: "Width" },
        PropertyDef { name: "h", kind: ValueKind::Length, required: false, doc: "Height" },
        PropertyDef { name: "caption", kind: ValueKind::String, required: false, doc: "Visible caption" },
    ],
    doc: "Image with required alt and optional caption",
};

// --- Lists ---

const OL: ElementSchema = ElementSchema {
    name: "ol",
    categories: &[BlockContainer],
    doc_types: &[Doc, Pres],
    bodies: &[Children],
    parents: &["any-block-container", "li"],
    children: &["li"],
    properties: &[
        PropertyDef { name: "style", kind: ValueKind::Style, required: false, doc: "Marker style" },
        PropertyDef { name: "start", kind: ValueKind::Integer, required: false, doc: "Starting position" },
    ],
    doc: "Ordered list",
};

const UL: ElementSchema = ElementSchema {
    name: "ul",
    categories: &[BlockContainer],
    doc_types: &[Doc, Pres],
    bodies: &[Children],
    parents: &["any-block-container", "li"],
    children: &["li"],
    properties: &[PropertyDef {
        name: "style",
        kind: ValueKind::Enum(&["disc", "circle", "square", "dash", "none"]),
        required: false,
        doc: "Bullet style",
    }],
    doc: "Unordered list",
};

const LI: ElementSchema = ElementSchema {
    name: "li",
    categories: &[BlockLeaf, BlockContainer],
    doc_types: &[Doc, Pres],
    bodies: &[Text, Children],
    parents: &["ol", "ul"],
    children: &["any-block"],
    properties: &[PropertyDef {
        name: "at",
        kind: ValueKind::Integer,
        required: false,
        doc: "Override this item's position",
    }],
    doc: "List item",
};

// --- Tables (document) ---

const TABLE: ElementSchema = ElementSchema {
    name: "table",
    categories: &[BlockContainer],
    doc_types: &[Doc, Pres],
    bodies: &[Children],
    parents: &["any-block-container"],
    children: &["row"],
    properties: &[
        PropertyDef {
            name: "border",
            kind: ValueKind::Enum(&["none", "outer", "all"]),
            required: false,
            doc: "Border policy",
        },
        PropertyDef { name: "stripe", kind: ValueKind::Bool, required: false, doc: "Alternate row backgrounds" },
        PropertyDef { name: "caption", kind: ValueKind::String, required: false, doc: "Table caption" },
    ],
    doc: "Document-style table",
};

const ROW_DOC: ElementSchema = ElementSchema {
    name: "row",
    categories: &[BlockContainer],
    doc_types: &[Doc, Pres],
    bodies: &[Children],
    parents: &["table"],
    children: &["cell"],
    properties: &[PropertyDef {
        name: "kind",
        kind: ValueKind::Enum(&["data", "header", "footer"]),
        required: false,
        doc: "Row role",
    }],
    doc: "Table row",
};

const CELL_DOC: ElementSchema = ElementSchema {
    name: "cell",
    categories: &[BlockLeaf],
    doc_types: &[Doc, Pres],
    bodies: &[Text],
    parents: &["row"],
    children: &[],
    properties: &[
        PropertyDef { name: "colspan", kind: ValueKind::Integer, required: false, doc: "Column span" },
        PropertyDef { name: "rowspan", kind: ValueKind::Integer, required: false, doc: "Row span" },
        PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
        PropertyDef {
            name: "align",
            kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
            required: false,
            doc: "Horizontal alignment",
        },
        PropertyDef {
            name: "valign",
            kind: ValueKind::Enum(&["top", "middle", "bottom"]),
            required: false,
            doc: "Vertical alignment",
        },
    ],
    doc: "Table cell",
};

// --- Presentation ---

const SLIDE: ElementSchema = ElementSchema {
    name: "slide",
    categories: &[BlockContainer],
    doc_types: &[Pres],
    bodies: &[Children],
    parents: &["root"],
    children: &["any-block"],
    properties: &[
        PropertyDef { name: "id", kind: ValueKind::String, required: false, doc: "Slide identifier" },
        PropertyDef { name: "layout", kind: ValueKind::String, required: false, doc: "Layout name" },
        PropertyDef { name: "background", kind: ValueKind::Color, required: false, doc: "Slide background" },
    ],
    doc: "Single slide",
};

const TITLE: ElementSchema = ElementSchema {
    name: "title",
    categories: &[BlockLeaf],
    doc_types: &[Pres],
    bodies: &[Text],
    parents: &["slide"],
    children: &[],
    properties: &[PropertyDef {
        name: "size",
        kind: ValueKind::Length,
        required: false,
        doc: "Override default title size",
    }],
    doc: "Slide title",
};

const BULLETS: ElementSchema = ElementSchema {
    name: "bullets",
    categories: &[BlockContainer],
    doc_types: &[Pres],
    bodies: &[Children],
    parents: &["slide", "col"],
    children: &["item"],
    properties: &[PropertyDef {
        name: "style",
        kind: ValueKind::Enum(&["disc", "dash", "arrow", "number"]),
        required: false,
        doc: "Marker style",
    }],
    doc: "Slide bullet list",
};

const ITEM: ElementSchema = ElementSchema {
    name: "item",
    categories: &[BlockLeaf, BlockContainer],
    doc_types: &[Pres],
    bodies: &[Text, Children],
    parents: &["bullets"],
    children: &["bullets"],
    properties: &[],
    doc: "Bullet item",
};

const SPEAKER_NOTE: ElementSchema = ElementSchema {
    name: "speaker-note",
    categories: &[BlockLeaf],
    doc_types: &[Pres],
    bodies: &[Text],
    parents: &["slide"],
    children: &[],
    properties: &[],
    doc: "Speaker notes",
};

const TRANSITION: ElementSchema = ElementSchema {
    name: "transition",
    categories: &[BlockMarker],
    doc_types: &[Pres],
    bodies: &[NoBody],
    parents: &["slide"],
    children: &[],
    properties: &[
        PropertyDef {
            name: "kind",
            kind: ValueKind::Enum(&["none", "fade", "slide-left", "slide-right", "zoom"]),
            required: true,
            doc: "Transition type",
        },
        PropertyDef { name: "duration", kind: ValueKind::Length, required: false, doc: "Duration" },
    ],
    doc: "Slide transition",
};

// --- Sheet ---

const SHEET: ElementSchema = ElementSchema {
    name: "sheet",
    categories: &[BlockContainer],
    doc_types: &[Sheet],
    bodies: &[Children],
    parents: &["root"],
    children: &["cell", "col", "row", "fill", "source", "named", "format", "chart"],
    properties: &[
        PropertyDef { name: "id", kind: ValueKind::String, required: false, doc: "Sheet id" },
        PropertyDef { name: "name", kind: ValueKind::String, required: false, doc: "Display name" },
        PropertyDef { name: "freeze", kind: ValueKind::Address, required: false, doc: "Freeze pane address" },
    ],
    doc: "Sheet tab",
};

const COL_SHEET: ElementSchema = ElementSchema {
    name: "col",
    categories: &[BlockMarker],
    doc_types: &[Sheet],
    bodies: &[NoBody],
    parents: &["sheet"],
    children: &[],
    properties: &[
        PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Column letter or range" },
        PropertyDef { name: "width", kind: ValueKind::Length, required: false, doc: "Column width" },
        PropertyDef {
            name: "fmt",
            kind: ValueKind::Enum(&["number", "currency", "percent", "date", "datetime", "text"]),
            required: false,
            doc: "Number format",
        },
    ],
    doc: "Sheet column-level properties",
};

const ROW_SHEET: ElementSchema = ElementSchema {
    name: "row",
    categories: &[BlockMarker],
    doc_types: &[Sheet],
    bodies: &[NoBody],
    parents: &["sheet"],
    children: &[],
    properties: &[
        PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Row number or range" },
        PropertyDef { name: "height", kind: ValueKind::Length, required: false, doc: "Row height" },
        PropertyDef {
            name: "weight",
            kind: ValueKind::Enum(&["light", "regular", "bold"]),
            required: false,
            doc: "Font weight",
        },
        PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
    ],
    doc: "Sheet row-level properties",
};

const CELL_SHEET: ElementSchema = ElementSchema {
    name: "cell",
    categories: &[BlockLeaf, BlockMarker],
    doc_types: &[Sheet],
    bodies: &[Text, NoBody],
    parents: &["sheet"],
    children: &[],
    properties: &[
        PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Cell address (A1, B5)" },
        PropertyDef {
            name: "fmt",
            kind: ValueKind::Enum(&["number", "currency", "percent", "date", "datetime", "text"]),
            required: false,
            doc: "Number format",
        },
        PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
        PropertyDef {
            name: "align",
            kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
            required: false,
            doc: "Horizontal alignment",
        },
        PropertyDef {
            name: "weight",
            kind: ValueKind::Enum(&["light", "regular", "bold"]),
            required: false,
            doc: "Font weight",
        },
    ],
    doc: "Sheet cell — value or formatting override",
};

const FILL: ElementSchema = ElementSchema {
    name: "fill",
    categories: &[BlockLeaf],
    doc_types: &[Sheet],
    bodies: &[Text],
    parents: &["sheet"],
    children: &[],
    properties: &[
        PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Top-left anchor" },
        PropertyDef { name: "sep", kind: ValueKind::String, required: false, doc: "Cell separator" },
        PropertyDef { name: "has-header", kind: ValueKind::Bool, required: false, doc: "Treat first row as header" },
    ],
    doc: "Bulk inline data (CSV)",
};

const SOURCE: ElementSchema = ElementSchema {
    name: "source",
    categories: &[BlockMarker],
    doc_types: &[Sheet],
    bodies: &[NoBody],
    parents: &["sheet"],
    children: &[],
    properties: &[
        PropertyDef { name: "file", kind: ValueKind::String, required: true, doc: "Path to CSV file" },
        PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Top-left anchor" },
        PropertyDef { name: "sep", kind: ValueKind::String, required: false, doc: "Cell separator" },
        PropertyDef { name: "has-header", kind: ValueKind::Bool, required: false, doc: "Treat first row as header" },
        PropertyDef { name: "encoding", kind: ValueKind::String, required: false, doc: "Source file encoding" },
    ],
    doc: "External CSV reference",
};

const NAMED: ElementSchema = ElementSchema {
    name: "named",
    categories: &[BlockMarker],
    doc_types: &[Sheet],
    bodies: &[NoBody],
    parents: &["sheet"],
    children: &[],
    properties: &[
        PropertyDef { name: "name", kind: ValueKind::String, required: true, doc: "Name for formulas" },
        PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Range" },
    ],
    doc: "Named range",
};

const FORMAT: ElementSchema = ElementSchema {
    name: "format",
    categories: &[BlockMarker],
    doc_types: &[Sheet],
    bodies: &[NoBody],
    parents: &["sheet"],
    children: &[],
    properties: &[
        PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Range" },
        PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
        PropertyDef {
            name: "weight",
            kind: ValueKind::Enum(&["light", "regular", "bold"]),
            required: false,
            doc: "Font weight",
        },
        PropertyDef {
            name: "align",
            kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
            required: false,
            doc: "Horizontal alignment",
        },
        PropertyDef {
            name: "fmt",
            kind: ValueKind::Enum(&["number", "currency", "percent", "date", "datetime", "text"]),
            required: false,
            doc: "Number format",
        },
    ],
    doc: "Range formatting without values",
};

const CHART: ElementSchema = ElementSchema {
    name: "chart",
    categories: &[BlockMarker],
    doc_types: &[Doc, Pres, Sheet],
    bodies: &[NoBody],
    parents: &["any-block-container"],
    children: &[],
    properties: &[
        PropertyDef {
            name: "type",
            kind: ValueKind::Enum(&["bar", "line", "pie", "scatter", "area"]),
            required: true,
            doc: "Chart type",
        },
        PropertyDef { name: "data", kind: ValueKind::String, required: true, doc: "Range ref" },
        PropertyDef { name: "title", kind: ValueKind::String, required: false, doc: "Chart title" },
    ],
    doc: "Chart from a data range",
};

// NOTE: `col`, `row`, `cell` are intentionally registered twice — once
// for document/presentation (layout/table semantics) and once for sheet
// (address semantics). `Registry::get(name, doc_type)` picks the
// matching variant.
const BUILTINS: &[ElementSchema] = &[
    // Universal inline
    TEXT, FOOTNOTE, CODE_EL, LINK, DATE, MENTION, MATH,
    // Document structural
    SECTION, LAYOUT, COL_LAYOUT, PAGEBREAK, HR,
    // Headings
    H1, H2, H3, H4, H5, H6,
    // Document block content
    P, NOTE, BLOCKQUOTE, IMAGE,
    // Lists
    OL, UL, LI,
    // Tables (document)
    TABLE, ROW_DOC, CELL_DOC,
    // Presentation
    SLIDE, TITLE, BULLETS, ITEM, SPEAKER_NOTE, TRANSITION,
    // Sheet — these intentionally come last so their values overwrite
    // the layout/table variants in the simple BTreeMap. Until the
    // multi-keyed lookup ships, the sheet validator wins ties.
    SHEET, COL_SHEET, ROW_SHEET, CELL_SHEET, FILL, SOURCE, NAMED, FORMAT, CHART,
];
