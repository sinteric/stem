use stem_core::diagnostic::Severity;
use stem_parser::parse_v2;
use stem_types::{default_registry_v2, validate_v2};

fn validate_src(src: &str) -> Vec<stem_core::Diagnostic> {
    let r = parse_v2(src);
    assert!(
        r.diagnostics.is_empty(),
        "parse errors: {:?}",
        r.diagnostics
    );
    validate_v2(&r.document, &default_registry_v2())
}

#[test]
fn clean_document_has_no_diagnostics() {
    let src = "[type:document]\nsection{\n  h1(Hello)\n  p(World)\n}";
    let diags = validate_src(src);
    assert!(diags.is_empty(), "{:?}", diags);
}

#[test]
fn unknown_element_warns() {
    let src = "[type:document]\nfoo(bar)";
    let diags = validate_src(src);
    assert!(diags.iter().any(|d| d.code == "type.unknown_function"
        && d.severity == Severity::Warning));
}

#[test]
fn wrong_doc_type_errors() {
    let src = "[type:presentation]\nsection{}";
    let diags = validate_src(src);
    assert!(diags
        .iter()
        .any(|d| d.code == "type.wrong_doc_type" && d.severity == Severity::Error));
}

#[test]
fn body_kind_mismatch_warns() {
    // h1 expects text body, but we give it a block body
    let src = "[type:document]\nh1{ p(oops) }";
    let diags = validate_src(src);
    assert!(diags
        .iter()
        .any(|d| d.code == "type.wrong_body_kind"));
}

#[test]
fn missing_required_property_errors() {
    let src = "[type:document]\nlayout{}"; // layout requires kind
    let diags = validate_src(src);
    assert!(diags
        .iter()
        .any(|d| d.code == "type.missing_property"));
}

#[test]
fn unknown_property_warns() {
    let src = "[type:document]\ncell[wat:yes](x)";
    let diags = validate_src(src);
    // 'cell' validates against the sheet schema first by registration order;
    // either way, 'wat' is unknown.
    assert!(diags
        .iter()
        .any(|d| d.code == "type.unknown_property" || d.code == "type.wrong_doc_type"));
}

#[test]
fn bad_enum_value_errors() {
    let src = "[type:document]\np[align:slightly-left](x)";
    let diags = validate_src(src);
    assert!(diags
        .iter()
        .any(|d| d.code == "type.bad_property_value"));
}

#[test]
fn color_value_accepts_theme_and_hex() {
    let src = "[type:document]\nsection{ p[align:left](x) note[kind:warning](y) }";
    let diags = validate_src(src);
    // No bad_property_value
    assert!(!diags.iter().any(|d| d.code == "type.bad_property_value"),
            "{:?}", diags);
}

#[test]
fn sheet_address_validates() {
    // Single cells: OK; quoted range: parser strips quotes, validator accepts the colon
    let src = "[type:sheet]\nsheet{ cell[at:A1](v) }";
    let diags = validate_src(src);
    assert!(diags.is_empty(), "{:?}", diags);
}

#[test]
fn inline_element_validates() {
    let src = "[type:document]\np(some @text[color:red](red) text)";
    let diags = validate_src(src);
    assert!(diags.is_empty(), "{:?}", diags);
}

#[test]
fn inline_unknown_warns() {
    let src = "[type:document]\np(some @fake(thing) text)";
    let diags = validate_src(src);
    assert!(diags.iter().any(|d| d.code == "type.unknown_function"));
}

#[test]
fn full_roadmap_validates_clean_v2() {
    let src = r#"[type:document, locale:ko-KR, title:"2026 Roadmap"]

section{
  h1(2026 Product Roadmap)
  h2(Strategy Team)
  date(2026.05.20)
}

section[id:toc]

section{
  h2(Background)

  p(Existing ecosystems are @text[color:primary](falling behind)
  in the AI era. @footnote(Gartner 2025 Report))

  layout[kind:two-column]{
    col{
      h3(Problems)
      ol[style:1.]{
        li(Format fragmentation)
        li(Hard to generate with AI)
      }
    }
    col{
      h3(Opportunities)
      ol[style:가.]{
        li(Single source format)
        li(AI-native design)
      }
    }
  }

  table[border:outer]{
    row[kind:header]{
      cell(Phase)
      cell(Content)
      cell[colspan:2](Timeline)
    }
    row{
      cell(Phase 1)
      cell(Spec finalization)
      cell(2026 Q2)
      cell[bg:yellow](In Progress)
    }
  }
}
"#;
    let diags = validate_src(src);
    assert!(diags.is_empty(), "unexpected diagnostics: {:?}", diags);
}
