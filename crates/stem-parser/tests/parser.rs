//! Parser tests. Cover the locked grammar rules from
//! `docs/grammar.md` and the structural constraints.

use stem_core::ast::*;
use stem_parser::parse;

fn parse_clean(src: &str) -> Document {
    let r = parse(src);
    assert!(
        r.diagnostics.is_empty(),
        "expected clean parse of {:?}, got diags: {:?}",
        src,
        r.diagnostics
    );
    r.document
}

// -----------------------------------------------------------
// Trivial cases
// -----------------------------------------------------------

#[test]
fn empty_input_produces_empty_doc() {
    let d = parse_clean("");
    assert!(d.blocks.is_empty());
    assert!(d.metadata.properties.is_empty());
}

#[test]
fn metadata_header_parses() {
    let d = parse_clean("[type:document, locale:ko-KR]");
    assert_eq!(d.metadata.properties.len(), 2);
    assert_eq!(d.metadata.get_str("type"), Some("document"));
    assert_eq!(d.metadata.get_str("locale"), Some("ko-KR"));
}

#[test]
fn bare_name_is_a_block() {
    let d = parse_clean("pagebreak");
    assert_eq!(d.blocks.len(), 1);
    assert_eq!(d.blocks[0].name, "pagebreak");
    assert!(matches!(d.blocks[0].body, Body::None));
    assert!(d.blocks[0].properties.is_empty());
}

#[test]
fn block_with_properties_no_body() {
    let d = parse_clean("section[id:toc]");
    assert_eq!(d.blocks.len(), 1);
    let b = &d.blocks[0];
    assert_eq!(b.name, "section");
    assert!(matches!(b.body, Body::None));
    assert_eq!(b.properties.len(), 1);
    assert_eq!(b.properties[0].key, "id");
    assert_eq!(b.properties[0].value.as_str(), "toc");
}

// -----------------------------------------------------------
// Text bodies
// -----------------------------------------------------------

#[test]
fn block_with_bare_text_body() {
    let d = parse_clean("h1(2026 Roadmap)");
    let b = &d.blocks[0];
    assert_eq!(b.name, "h1");
    assert_eq!(b.plain_text().as_deref(), Some("2026 Roadmap"));
}

#[test]
fn block_with_quoted_text_body() {
    let d = parse_clean(r#"cell[at:B5]("=SUM(B2:B4)")"#);
    let b = &d.blocks[0];
    assert_eq!(b.name, "cell");
    assert_eq!(b.plain_text().as_deref(), Some("=SUM(B2:B4)"));
    // Value should be quoted-form preserved
    assert_eq!(b.prop_str("at"), Some("B5"));
}

#[test]
fn unicode_escape_in_quoted_text() {
    let d = parse_clean(r#"note("zero-width \u{200B} space")"#);
    let b = &d.blocks[0];
    assert_eq!(b.plain_text().as_deref(), Some("zero-width \u{200B} space"));
}

#[test]
fn unicode_escape_in_bare_text() {
    let d = parse_clean(r"note(zero-width \u{200B} space)");
    let b = &d.blocks[0];
    assert_eq!(b.plain_text().as_deref(), Some("zero-width \u{200B} space"));
}

#[test]
fn escape_paren_in_bare_text() {
    let d = parse_clean(r"note(weight \(kg\))");
    let b = &d.blocks[0];
    assert_eq!(b.plain_text().as_deref(), Some("weight (kg)"));
}

#[test]
fn escape_at_in_bare_text() {
    let d = parse_clean(r"p(email \@host)");
    let b = &d.blocks[0];
    assert_eq!(b.plain_text().as_deref(), Some("email @host"));
}

#[test]
fn quoted_string_double_quote_doubling() {
    let d = parse_clean(r#"p("He said ""hi"".")"#);
    let b = &d.blocks[0];
    assert_eq!(b.plain_text().as_deref(), Some(r#"He said "hi"."#));
}

#[test]
fn unicode_preserved_in_bare_text() {
    let d = parse_clean("p(한국어 테스트)");
    let b = &d.blocks[0];
    assert_eq!(b.plain_text().as_deref(), Some("한국어 테스트"));
}

// -----------------------------------------------------------
// Block bodies
// -----------------------------------------------------------

#[test]
fn block_body_with_children() {
    let src = "section{\n  h1(Title)\n  p(Body.)\n}";
    let d = parse_clean(src);
    let s = &d.blocks[0];
    assert_eq!(s.name, "section");
    let children = match &s.body {
        Body::Children(c) => c,
        _ => panic!("expected children body"),
    };
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].name, "h1");
    assert_eq!(children[1].name, "p");
}

#[test]
fn empty_block_body_is_silent() {
    let r = parse("section{}");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let s = &r.document.blocks[0];
    assert!(matches!(&s.body, Body::Children(v) if v.is_empty()));
}

#[test]
fn nested_block_bodies() {
    let src = "layout[kind:two-column]{\n  col{ h3(Left) }\n  col{ h3(Right) }\n}";
    let d = parse_clean(src);
    let layout = &d.blocks[0];
    assert_eq!(layout.name, "layout");
    let children = match &layout.body {
        Body::Children(c) => c,
        _ => panic!(),
    };
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].name, "col");
    assert_eq!(children[1].name, "col");
}

// -----------------------------------------------------------
// Inline elements
// -----------------------------------------------------------

#[test]
fn inline_element_inside_text_body() {
    let d = parse_clean("p(The @text[color:red](critical) issue.)");
    let p = &d.blocks[0];
    let pieces = match &p.body {
        Body::Text(t) => t,
        _ => panic!(),
    };
    assert_eq!(pieces.len(), 3);
    match &pieces[0] {
        TextPiece::Literal { text, .. } => assert_eq!(text, "The "),
        _ => panic!(),
    }
    match &pieces[1] {
        TextPiece::Inline(b) => {
            assert_eq!(b.name, "text");
            assert!(b.inline_form);
            assert_eq!(b.prop_str("color"), Some("red"));
            assert_eq!(b.plain_text().as_deref(), Some("critical"));
        }
        _ => panic!(),
    }
    match &pieces[2] {
        TextPiece::Literal { text, .. } => assert_eq!(text, " issue."),
        _ => panic!(),
    }
}

#[test]
fn bare_at_ident_is_inline_call_not_prose() {
    // `alert(1)` in prose is literal (no `@`); but `@alert(1)` IS a call
    let d = parse_clean("p(@alert(1))");
    let p = &d.blocks[0];
    let pieces = match &p.body {
        Body::Text(t) => t,
        _ => panic!(),
    };
    assert_eq!(pieces.len(), 1);
    let inline = match &pieces[0] {
        TextPiece::Inline(b) => b,
        _ => panic!(),
    };
    assert_eq!(inline.name, "alert");
    assert!(inline.inline_form);
    assert_eq!(inline.plain_text().as_deref(), Some("1"));
}

// -----------------------------------------------------------
// Diagnostics — structural violations
// -----------------------------------------------------------

#[test]
fn bare_open_paren_in_text_is_error() {
    let r = parse("p(He said (hi))");
    assert!(r.diagnostics.iter().any(|d| d.code == "parse.bad_escape"));
}

#[test]
fn top_level_text_is_error() {
    let r = parse("(hello)");
    assert!(r.diagnostics.iter().any(|d| d.code == "parse.top_level_text"));
}

#[test]
fn bodyless_inline_is_error() {
    let r = parse("p(see @ref here)");
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.code == "parse.bodyless_inline_required"));
}

#[test]
fn multiple_bodies_is_error() {
    let r = parse("p(text)(more)");
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.code == "parse.multiple_bodies"));
}

#[test]
fn misplaced_properties_after_body_is_error() {
    let r = parse("p(text)[color:red]");
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.code == "parse.misplaced_properties"));
}

#[test]
fn bare_property_value_with_colon_is_error() {
    let r = parse("section[at:B2:B4]");
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.code == "parse.bad_property_value"));
}

#[test]
fn quoted_property_value_with_colon_is_ok() {
    let d = parse_clean(r#"section[at:"B2:B4"]"#);
    assert_eq!(d.blocks[0].prop_str("at"), Some("B2:B4"));
}

#[test]
fn unclosed_paren_emits_diagnostic_and_recovers() {
    let r = parse("h1(hello");
    assert!(r.diagnostics.iter().any(|d| d.code == "parse.unclosed_paren"));
}

#[test]
fn empty_text_body_emits_hint() {
    let r = parse("section()");
    use stem_core::diagnostic::Severity;
    assert!(r.diagnostics.iter().any(|d| {
        d.code == "parse.empty_text_body" && d.severity == Severity::Hint
    }));
}

#[test]
fn invalid_unicode_codepoint_errors() {
    let r = parse(r#"p("bad: \u{D800}")"#);
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.code == "parse.invalid_codepoint"));
}

#[test]
fn line_comment_is_skipped() {
    let d = parse_clean("// header comment\nh1(Hello)\n// trailing");
    assert_eq!(d.blocks.len(), 1);
    assert_eq!(d.blocks[0].name, "h1");
}

// -----------------------------------------------------------
// Roadmap-style integration
// -----------------------------------------------------------

#[test]
fn roadmap_section_parses_clean() {
    let src = "[type:document, locale:ko-KR]\n\nsection{\n  h1(2026 Roadmap)\n  h2(Strategy Team)\n  date(2026.05.20)\n}\n\nsection[id:toc]\n";
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let d = r.document;
    assert_eq!(d.blocks.len(), 2);
    assert_eq!(d.blocks[0].name, "section");
    assert_eq!(d.blocks[1].name, "section");
    assert_eq!(d.blocks[1].prop_str("id"), Some("toc"));
}

#[test]
fn sheet_fill_quoted_body() {
    let src = r#"sheet[id:Q4]{
  fill[at:A1]("
    Item, Revenue
    Widget, 42000
  ")
  cell[at:C5, bg:yellow]
}"#;
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let sheet = &r.document.blocks[0];
    assert_eq!(sheet.name, "sheet");
    let children = match &sheet.body {
        Body::Children(c) => c,
        _ => panic!(),
    };
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].name, "fill");
    assert_eq!(children[1].name, "cell");
    assert_eq!(children[1].prop_str("bg"), Some("yellow"));
}
