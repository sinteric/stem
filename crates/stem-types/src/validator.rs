//! Walk the AST, validate against the registry, emit diagnostics.

use stem_core::ast::*;
use stem_core::diagnostic::Diagnostic;
use stem_core::theme::Theme;

use crate::schema::{ArgArity, DocumentType, Registry, ValueKind};

pub fn validate(doc: &Document, registry: &Registry) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    // Resolve the document type from metadata (defaults to Document).
    let doc_type = doc
        .metadata
        .get_str("type")
        .and_then(DocumentType::from_str)
        .unwrap_or(DocumentType::Document);

    if let Some(type_str) = doc.metadata.get_str("type") {
        if DocumentType::from_str(type_str).is_none() {
            // Best-effort: span the metadata header itself.
            out.push(Diagnostic::warning(
                "type.unknown_doc_type",
                format!("unknown document type `{}`", type_str),
                doc.metadata.span,
            ));
        }
    }

    // Validate every top-level node.
    for node in &doc.nodes {
        if let Node::Call(c) = node {
            validate_call(c, registry, doc_type, &mut out);
        }
    }

    out
}

fn validate_call(
    call: &FunctionCall,
    registry: &Registry,
    doc_type: DocumentType,
    out: &mut Vec<Diagnostic>,
) {
    let schema = match registry.get(&call.name) {
        Some(s) => s,
        None => {
            out.push(Diagnostic::error(
                "type.unknown_function",
                format!("unknown function `{}`", call.name),
                call.name_span,
            ));
            // Still validate children so other diagnostics surface.
            for group in &call.args {
                for c in group {
                    if let Content::Call(child) = c {
                        validate_call(child, registry, doc_type, out);
                    }
                }
            }
            return;
        }
    };

    // Document type filter.
    if !schema.allowed_in.is_empty() && !schema.allowed_in.contains(&doc_type) {
        let allowed: Vec<&str> = schema.allowed_in.iter().map(|t| t.as_str()).collect();
        out.push(Diagnostic::error(
            "type.wrong_doc_type",
            format!(
                "`{}` is not valid in document type `{}` (allowed in: {})",
                call.name,
                doc_type.as_str(),
                allowed.join(", "),
            ),
            call.name_span,
        ));
    }

    // Argument arity.
    let n = call.args.len() as u8;
    let arity_ok = match schema.arity {
        ArgArity::Exact(want) => n == want,
        ArgArity::Range(lo, hi) => n >= lo && n <= hi,
        ArgArity::Any => true,
    };
    if !arity_ok {
        let want = match schema.arity {
            ArgArity::Exact(w) => format!("exactly {} argument group(s)", w),
            ArgArity::Range(lo, hi) => format!("between {} and {} argument group(s)", lo, hi),
            ArgArity::Any => "any number of argument groups".to_string(),
        };
        out.push(Diagnostic::error(
            "type.wrong_arity",
            format!("`{}` expects {}, got {}", call.name, want, n),
            call.span,
        ));
    }

    // (No block/inline diagnostic here: the renderer's cook stage
    // already lifts known block-preferred names to block-level, so a
    // user-facing hint would be noise rather than signal.)

    // Properties.
    let theme = Theme::default();
    for prop in &call.properties {
        let schema_prop = schema.properties.iter().find(|p| p.name == prop.key);
        match schema_prop {
            None => {
                out.push(Diagnostic::warning(
                    "type.unknown_property",
                    format!("unknown property `{}` on `{}`", prop.key, call.name),
                    prop.key_span,
                ));
            }
            Some(p) => {
                let raw = prop.value.as_str();
                let ok = match &p.kind {
                    ValueKind::String => true,
                    ValueKind::Integer => raw.parse::<i64>().is_ok(),
                    ValueKind::Bool => prop.value.as_bool().is_some(),
                    ValueKind::Enum(vals) => vals.contains(&raw),
                    ValueKind::Color => theme.resolve_color(raw).is_some(),
                };
                if !ok {
                    let want = match &p.kind {
                        ValueKind::String => "a string".to_string(),
                        ValueKind::Integer => "an integer".to_string(),
                        ValueKind::Bool => "true/false".to_string(),
                        ValueKind::Enum(vals) => format!("one of [{}]", vals.join(", ")),
                        ValueKind::Color => "a theme color name or `#rrggbb`".to_string(),
                    };
                    out.push(Diagnostic::error(
                        "type.bad_property_value",
                        format!(
                            "property `{}` on `{}` expected {}, got `{}`",
                            prop.key, call.name, want, raw
                        ),
                        prop.value_span,
                    ));
                }
            }
        }
    }

    // Required props.
    for p in schema.properties {
        if p.required && !call.properties.iter().any(|pp| pp.key == p.name) {
            out.push(Diagnostic::error(
                "type.missing_property",
                format!("required property `{}` missing on `{}`", p.name, call.name),
                call.name_span,
            ));
        }
    }

    // Recurse into children.
    for group in &call.args {
        for c in group {
            if let Content::Call(child) = c {
                validate_call(child, registry, doc_type, out);
            }
        }
    }
}
