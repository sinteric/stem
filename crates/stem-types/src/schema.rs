//! Schema registry. Mirrors `docs/schema.md`.
//!
//! Hand-keyed for now. Once `docs/schema.md`'s `stem-schema` blocks
//! can be machine-extracted, this module will load them at startup
//! instead — design is identical, just the source of truth moves.

use std::collections::BTreeMap;

/// Document type — the named kind of document Stem is processing.
///
/// `Document`, `Presentation`, and `Sheet` are the three built-in
/// kinds. Embedders that ship custom doc types (mindmaps, whiteboards,
/// diagrams, etc.) construct [`DocumentType::Custom`] with a
/// `&'static str` name and register elements with `doc_types: &[Custom("…")]`.
///
/// Names are case-sensitive. Built-in names are lowercase; embedders
/// should follow the same convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocumentType {
    Document,
    Presentation,
    Sheet,
    /// Embedder-defined doc type. The string is the name surfaced in
    /// `type:<name>` metadata and used for `doc_types` matching.
    Custom(&'static str),
}

impl DocumentType {
    /// Parse a doc-type name from the `type:` metadata.
    ///
    /// Returns `None` for unknown names. Embedders that want to accept
    /// custom doc types should call [`DocumentType::custom`] explicitly
    /// rather than relying on `from_str`, since `from_str` has no way
    /// to leak a static lifetime from a runtime string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "document" => Some(Self::Document),
            "presentation" => Some(Self::Presentation),
            "sheet" => Some(Self::Sheet),
            _ => None,
        }
    }

    /// Construct a custom doc type. The name must be a static string
    /// (typically a `const` in the embedder's code).
    pub const fn custom(name: &'static str) -> Self {
        Self::Custom(name)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Presentation => "presentation",
            Self::Sheet => "sheet",
            Self::Custom(s) => s,
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

    /// Resolve a doc-type name from `type:` metadata, looking up
    /// embedder-registered custom doc types. Built-in names short-circuit
    /// to the corresponding variant; otherwise the registry is scanned
    /// for an element that declares `doc_types: &[Custom(name)]`.
    ///
    /// Returns `None` for names neither built-in nor registered. The
    /// validator surfaces this as `type.unknown_doc_type`.
    pub fn resolve_doc_type(&self, name: &str) -> Option<DocumentType> {
        if let Some(dt) = DocumentType::from_str(name) {
            return Some(dt);
        }
        for variants in self.by_name.values() {
            for v in variants {
                for &dt in v.doc_types {
                    if let DocumentType::Custom(s) = dt {
                        if s == name {
                            return Some(dt);
                        }
                    }
                }
            }
        }
        None
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
    for def in crate::elements::ALL {
        r.insert(def.schema.clone());
    }
    r
}

