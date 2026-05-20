//! Parser tests covering the trickier corners of the grammar.

use stem_core::ast::*;
use stem_parser::parse;

fn parse_doc(src: &str) -> Document {
    let r = parse(src);
    assert!(
        r.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        r.diagnostics
    );
    r.document
}

#[test]
fn empty_input() {
    let d = parse_doc("");
    assert!(d.metadata.properties.is_empty());
    assert!(d.nodes.is_empty());
}

#[test]
fn metadata_header_parses() {
    let d = parse_doc("[type:document, encoding:utf-8, locale:ko-KR]\n");
    let m = &d.metadata;
    assert_eq!(m.properties.len(), 3);
    assert_eq!(m.properties[0].key, "type");
    assert_eq!(m.properties[0].value.as_str(), "document");
    assert_eq!(m.properties[1].key, "encoding");
    assert_eq!(m.properties[1].value.as_str(), "utf-8");
    assert_eq!(m.properties[2].key, "locale");
    assert_eq!(m.properties[2].value.as_str(), "ko-KR");
}

#[test]
fn quoted_property_value() {
    let d = parse_doc("[title:\"Hello, World\"]\n");
    let v = &d.metadata.properties[0].value;
    assert!(matches!(v, PropertyValue::String(s) if s == "Hello, World"));
}

#[test]
fn inline_call_keeps_inline_kind() {
    let d = parse_doc("This has text(red bits)[color:red] inline.");
    // text run "This has ", call, text run " inline."
    assert_eq!(d.nodes.len(), 3);
    let call = match &d.nodes[1] {
        Node::Call(c) => c,
        n => panic!("expected call, got {:?}", n),
    };
    assert_eq!(call.name, "text");
    assert_eq!(call.kind, CallKind::Inline);
    assert_eq!(call.properties.len(), 1);
    assert_eq!(call.properties[0].key, "color");
    assert_eq!(call.properties[0].value.as_str(), "red");
}

#[test]
fn chained_args_section_with_body() {
    // `section(cover)(body)` is one call with two arg groups. Since the
    // second group contains a newline at depth 0, kind is Block.
    let d = parse_doc("section(cover)(\n  hello\n)");
    assert_eq!(d.nodes.len(), 1);
    let outer = match &d.nodes[0] {
        Node::Call(c) => c,
        n => panic!("expected call, got {:?}", n),
    };
    assert_eq!(outer.name, "section");
    assert_eq!(outer.kind, CallKind::Block);
    assert_eq!(outer.args.len(), 2);
    // header is "cover"
    let header = outer.header().expect("expected header arg");
    let header_text = match &header[0] {
        Content::Text(t) => t.text.as_str(),
        _ => panic!(),
    };
    assert_eq!(header_text, "cover");
    // body contains "hello"
    let body = outer.body();
    let body_text = match &body[0] {
        Content::Text(t) => t.text.as_str(),
        _ => panic!(),
    };
    assert!(body_text.contains("hello"));
}

#[test]
fn nested_balanced_parens_in_text() {
    // The space before `(bar)` means it's literal, not a function call.
    let d = parse_doc("cell(foo (bar) baz)");
    let call = match &d.nodes[0] {
        Node::Call(c) => c,
        _ => panic!(),
    };
    assert_eq!(call.name, "cell");
    assert_eq!(call.body().len(), 1);
    let text = match &call.body()[0] {
        Content::Text(t) => &t.text,
        _ => panic!(),
    };
    assert_eq!(text, "foo (bar) baz");
}

#[test]
fn nested_call_disambiguated() {
    let d = parse_doc("cell(foo bar(baz) qux)");
    let outer = match &d.nodes[0] {
        Node::Call(c) => c,
        _ => panic!(),
    };
    assert_eq!(outer.body().len(), 3);
    match &outer.body()[0] {
        Content::Text(t) => assert_eq!(t.text, "foo "),
        _ => panic!(),
    }
    match &outer.body()[1] {
        Content::Call(c) => {
            assert_eq!(c.name, "bar");
            assert_eq!(c.body().len(), 1);
            let inner_text = match &c.body()[0] {
                Content::Text(t) => &t.text,
                _ => panic!(),
            };
            assert_eq!(inner_text, "baz");
        }
        _ => panic!(),
    }
    match &outer.body()[2] {
        Content::Text(t) => assert_eq!(t.text, " qux"),
        _ => panic!(),
    }
}

#[test]
fn escape_handles_literal_parens() {
    let d = parse_doc(r"note(weight \(kg\))");
    let call = match &d.nodes[0] {
        Node::Call(c) => c,
        _ => panic!(),
    };
    let text = match &call.body()[0] {
        Content::Text(t) => &t.text,
        _ => panic!(),
    };
    assert_eq!(text, "weight (kg)");
}

#[test]
fn unclosed_paren_emits_diagnostic_and_recovers() {
    let r = parse("section(\n  hello");
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].code, "parse.unclosed_paren");
    // We still got a section node with content
    let call = match &r.document.nodes[0] {
        Node::Call(c) => c,
        n => panic!("expected call, got {:?}", n),
    };
    assert_eq!(call.name, "section");
}

#[test]
fn unicode_in_text_runs_preserved() {
    let d = parse_doc("note(한국어 테스트)");
    let call = match &d.nodes[0] {
        Node::Call(c) => c,
        _ => panic!(),
    };
    let text = match &call.body()[0] {
        Content::Text(t) => &t.text,
        _ => panic!(),
    };
    assert_eq!(text, "한국어 테스트");
}

#[test]
fn nested_block_inside_block() {
    let src = "section(cover)\nlayout(two-column)(\n  col(left content)\n  col(right content)\n)";
    let d = parse_doc(src);
    // top-level: Call(section), Text("\n"), Call(layout)
    let layout = match d.nodes.iter().find_map(|n| match n {
        Node::Call(c) if c.name == "layout" => Some(c),
        _ => None,
    }) {
        Some(c) => c,
        None => panic!("no layout call in {:?}", d.nodes),
    };
    assert_eq!(layout.kind, CallKind::Block);
    // layout body should have two col() calls
    let col_count = layout
        .body()
        .iter()
        .filter(|c| matches!(c, Content::Call(c) if c.name == "col"))
        .count();
    assert_eq!(col_count, 2);
}

#[test]
fn property_after_block_call() {
    let d = parse_doc("table(\n  row(cell(a))\n)[border:outer]");
    let table = match &d.nodes[0] {
        Node::Call(c) => c,
        _ => panic!(),
    };
    assert_eq!(table.kind, CallKind::Block);
    assert_eq!(table.properties.len(), 1);
    assert_eq!(table.properties[0].key, "border");
    assert_eq!(table.properties[0].value.as_str(), "outer");
}

#[test]
fn ident_in_text_without_paren_is_literal() {
    // "section is" — "section" is an ident but not followed by `(`, so literal.
    let d = parse_doc("the section is small");
    assert_eq!(d.nodes.len(), 1);
    match &d.nodes[0] {
        Node::Text(t) => assert_eq!(t.text, "the section is small"),
        _ => panic!(),
    }
}

#[test]
fn cook_paragraph_and_heading() {
    let src = "section(body)(\n  # Title\n\n  Paragraph one.\n  More of paragraph one.\n)";
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let cooked = stem_parser::cook_document(&r.document);
    // The top-level is one Block::Call (the `section`).
    let section_call = match &cooked.blocks[0] {
        Block::Call(c) => c,
        b => panic!("expected Block::Call, got {:?}", b),
    };
    let inner = stem_parser::cook_call_content(section_call);
    // Should contain at least: Heading "Title", Paragraph(...)
    assert!(
        inner
            .iter()
            .any(|b| matches!(b, Block::Heading { level: 1, .. })),
        "no H1 in cooked output: {:?}",
        inner
    );
    assert!(
        inner.iter().any(|b| matches!(b, Block::Paragraph(_))),
        "no paragraph in cooked output: {:?}",
        inner
    );
}

#[test]
fn cook_unordered_list() {
    let src = "section(b)(\n  - alpha\n  - beta\n  - gamma\n)";
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let cooked = stem_parser::cook_document(&r.document);
    let section_call = match &cooked.blocks[0] {
        Block::Call(c) => c,
        _ => panic!(),
    };
    let inner = stem_parser::cook_call_content(section_call);
    let list = inner.iter().find_map(|b| match b {
        Block::List { kind, items, .. } => Some((kind, items)),
        _ => None,
    });
    let (kind, items) = list.expect("no list found");
    assert_eq!(*kind, ListKind::Unordered);
    assert_eq!(items.len(), 3);
}
