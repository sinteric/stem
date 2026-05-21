//! Grammar v2 validator. Walks a `Document` from `stem_core::ast_v2`
//! and emits diagnostics against a `Registry` from `schema_v2`.

use stem_core::ast_v2::*;
use stem_core::diagnostic::Diagnostic;
use stem_core::theme::Theme;

use crate::schema_v2::{BodyKind, DocumentType, PropertyDef, Registry, ValueKind};

pub fn validate(doc: &Document, registry: &Registry) -> Vec<Diagnostic> {
    let doc_type = doc
        .metadata
        .get_str("type")
        .and_then(DocumentType::from_str)
        .unwrap_or(DocumentType::Document);

    let mut out = Vec::new();
    if let Some(t) = doc.metadata.get_str("type") {
        if DocumentType::from_str(t).is_none() {
            out.push(Diagnostic::warning(
                "type.unknown_doc_type",
                format!("unknown document type `{}`", t),
                doc.metadata.span,
            ));
        }
    }

    for block in &doc.blocks {
        validate_block(block, registry, doc_type, &mut out);
    }
    out
}

fn validate_block(
    block: &Block,
    registry: &Registry,
    doc_type: DocumentType,
    out: &mut Vec<Diagnostic>,
) {
    let schema = match registry.get(&block.name, doc_type) {
        Some(s) => s,
        None => {
            // Element name might exist for a different doc type — emit a
            // doc-type-aware diagnostic in that case.
            if registry.has_any(&block.name) {
                out.push(Diagnostic::error(
                    "type.wrong_doc_type",
                    format!(
                        "`{}` is not valid in document type `{}`",
                        block.name,
                        doc_type.as_str()
                    ),
                    block.name_span,
                ));
            } else {
                out.push(Diagnostic::warning(
                    "type.unknown_function",
                    format!("unknown element `{}`", block.name),
                    block.name_span,
                ));
            }
            recurse(block, registry, doc_type, out);
            return;
        }
    };

    // Body kind match
    let actual = match &block.body {
        Body::None => BodyKind::None,
        Body::Text(_) => BodyKind::Text,
        Body::Children(_) => BodyKind::Children,
    };
    if !schema.bodies.contains(&actual) {
        let allowed: Vec<&str> = schema
            .bodies
            .iter()
            .map(|b| match b {
                BodyKind::None => "no body",
                BodyKind::Text => "text body `(…)`",
                BodyKind::Children => "block body `{…}`",
            })
            .collect();
        let got = match actual {
            BodyKind::None => "no body",
            BodyKind::Text => "text body",
            BodyKind::Children => "block body",
        };
        out.push(Diagnostic::warning(
            "type.wrong_body_kind",
            format!(
                "`{}` expects {}, got {}",
                block.name,
                allowed.join(" or "),
                got,
            ),
            block.span,
        ));
    }

    // Property validation
    let theme = Theme::default();
    for prop in &block.properties {
        match schema.properties.iter().find(|p| p.name == prop.key) {
            None => {
                out.push(Diagnostic::warning(
                    "type.unknown_property",
                    format!("unknown property `{}` on `{}`", prop.key, block.name),
                    prop.key_span,
                ));
            }
            Some(def) => {
                if let Some(diag) = check_value(def, prop, &theme) {
                    out.push(diag);
                }
            }
        }
    }
    for def in schema.properties {
        if def.required && !block.properties.iter().any(|p| p.key == def.name) {
            out.push(Diagnostic::error(
                "type.missing_property",
                format!("required property `{}` missing on `{}`", def.name, block.name),
                block.name_span,
            ));
        }
    }

    recurse(block, registry, doc_type, out);
}

fn recurse(
    block: &Block,
    registry: &Registry,
    doc_type: DocumentType,
    out: &mut Vec<Diagnostic>,
) {
    match &block.body {
        Body::None => {}
        Body::Children(kids) => {
            for k in kids {
                validate_block(k, registry, doc_type, out);
            }
        }
        Body::Text(pieces) => {
            for p in pieces {
                if let TextPiece::Inline(inline) = p {
                    validate_block(inline, registry, doc_type, out);
                }
            }
        }
    }
}

fn check_value(def: &PropertyDef, prop: &Property, theme: &Theme) -> Option<Diagnostic> {
    let raw = prop.value.as_str();
    let ok = match &def.kind {
        ValueKind::String => true,
        ValueKind::Integer => raw.parse::<i64>().is_ok(),
        ValueKind::Bool => prop.value.as_bool().is_some(),
        ValueKind::Color => theme.resolve_color(raw).is_some(),
        ValueKind::Length => is_length(raw),
        ValueKind::Address => is_address(raw),
        ValueKind::Style => true, // list-marker styles are a small open set; accept any string
        ValueKind::Enum(vals) => vals.iter().any(|v| *v == raw),
    };
    if ok {
        return None;
    }
    let want = match &def.kind {
        ValueKind::String => "a string".to_string(),
        ValueKind::Integer => "an integer".to_string(),
        ValueKind::Bool => "true/false".to_string(),
        ValueKind::Color => "a theme color name or `#rrggbb`".to_string(),
        ValueKind::Length => "a length (e.g. 12pt, 60%, 100px, auto)".to_string(),
        ValueKind::Address => "an address (A1, B, 5) or quoted range (\"B2:B4\")".to_string(),
        ValueKind::Style => "a marker style".to_string(),
        ValueKind::Enum(vals) => format!("one of [{}]", vals.join(", ")),
    };
    Some(Diagnostic::error(
        "type.bad_property_value",
        format!(
            "property `{}` on the call expected {}, got `{}`",
            prop.key, want, raw
        ),
        prop.value_span,
    ))
}

fn is_length(s: &str) -> bool {
    if s == "auto" {
        return true;
    }
    // Accept N, N.M with optional unit: px, pt, em, rem, %, vw, vh
    let units = ["px", "pt", "em", "rem", "%", "vw", "vh"];
    if let Some(num) = units.iter().find_map(|u| s.strip_suffix(u)) {
        return num.parse::<f64>().is_ok();
    }
    s.parse::<f64>().is_ok()
}

fn is_address(s: &str) -> bool {
    // Single cell: letter(s) + digit(s) — e.g. A1, AB123
    // Whole column: letter(s) — e.g. B, AA
    // Whole row: digit(s) — e.g. 5, 123
    // Range: anything with a `:` (must be quoted; the parser strips quotes by the time we see this)
    if s.is_empty() {
        return false;
    }
    if s.contains(':') {
        // crude range validation: both sides look address-like
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return false;
        }
        return is_simple_address(parts[0]) && is_simple_address(parts[1]);
    }
    is_simple_address(s)
}

fn is_simple_address(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    let letters = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    // entire string consumed, and at least one letter or one digit
    i == bytes.len() && (letters > 0 || i > 0)
}
