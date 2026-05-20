use stem_core::diagnostic::Severity;
use stem_parser::parse;
use stem_types::{default_registry, validate};

fn validate_src(src: &str) -> Vec<stem_core::Diagnostic> {
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "parse errors: {:?}", r.diagnostics);
    validate(&r.document, &default_registry())
}

#[test]
fn clean_document_has_no_diagnostics() {
    let src = "[type:document]\nsection(cover)(\n  hello\n)";
    let diags = validate_src(src);
    assert!(diags.is_empty(), "{:?}", diags);
}

#[test]
fn unknown_function_errors() {
    let src = "[type:document]\nfoo(bar)";
    let diags = validate_src(src);
    assert!(diags.iter().any(|d| d.code == "type.unknown_function"));
}

#[test]
fn wrong_document_type_errors() {
    let src = "[type:presentation]\nsection(cover)";
    let diags = validate_src(src);
    assert!(diags.iter().any(|d| d.code == "type.wrong_doc_type"
        && d.severity == Severity::Error));
}

#[test]
fn unknown_property_warns() {
    let src = "[type:document]\ncell(x)[wat:y]";
    let diags = validate_src(src);
    assert!(diags.iter().any(|d| d.code == "type.unknown_property"));
}

#[test]
fn bad_enum_value_errors() {
    let src = "[type:document]\ncell(x)[align:slightly-left]";
    let diags = validate_src(src);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "type.bad_property_value"),
        "{:?}",
        diags
    );
}

#[test]
fn color_accepts_theme_and_hex() {
    let src = "[type:document]\ncell(x)[bg:#ff00aa]\ncell(y)[bg:primary]";
    let diags = validate_src(src);
    assert!(
        !diags.iter().any(|d| d.code == "type.bad_property_value"),
        "{:?}",
        diags
    );
}

#[test]
fn missing_required_property_errors() {
    // chart requires a `type` property
    let src = "[type:document]\nchart(data)";
    let diags = validate_src(src);
    assert!(
        diags.iter().any(|d| d.code == "type.missing_property"),
        "{:?}",
        diags
    );
}
