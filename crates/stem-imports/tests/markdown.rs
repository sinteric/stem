use stem_core::ast::{Body, TextPiece};
use stem_core::Exporter;
use stem_exports::HtmlExporter;
use stem_imports::markdown::import_str;

#[test]
fn heading_imports() {
    let doc = import_str("# Hello\n");
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.blocks[0].name, "h1");
    let text = doc.blocks[0].plain_text().expect("h1 should have plain text");
    assert_eq!(text.trim(), "Hello");
}

#[test]
fn multiple_heading_levels() {
    let doc = import_str("# H1\n\n## H2\n\n### H3\n");
    let names: Vec<&str> = doc.blocks.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, vec!["h1", "h2", "h3"]);
}

#[test]
fn paragraph_with_inline_emphasis() {
    let doc = import_str("This is *italic* and **bold**.\n");
    assert_eq!(doc.blocks.len(), 1);
    let p = &doc.blocks[0];
    assert_eq!(p.name, "p");
    // body should be Text with mixed literal and inline @text pieces
    if let Body::Text(pieces) = &p.body {
        let has_italic = pieces.iter().any(|pc| matches!(
            pc,
            TextPiece::Inline(b) if b.name == "text"
                && b.prop_str("style") == Some("italic")
        ));
        let has_bold = pieces.iter().any(|pc| matches!(
            pc,
            TextPiece::Inline(b) if b.name == "text"
                && b.prop_str("weight") == Some("bold")
        ));
        assert!(has_italic, "expected an italic @text inline");
        assert!(has_bold, "expected a bold @text inline");
    } else {
        panic!("expected Text body");
    }
}

#[test]
fn ordered_list_with_items() {
    let doc = import_str("1. First\n2. Second\n3. Third\n");
    assert_eq!(doc.blocks.len(), 1);
    let ol = &doc.blocks[0];
    assert_eq!(ol.name, "ol");
    if let Body::Children(items) = &ol.body {
        assert_eq!(items.len(), 3);
        for (i, expected) in ["First", "Second", "Third"].iter().enumerate() {
            assert_eq!(items[i].name, "li");
            assert_eq!(items[i].plain_text().unwrap().trim(), *expected);
        }
    } else {
        panic!("expected Children body");
    }
}

#[test]
fn unordered_list() {
    let doc = import_str("- apple\n- banana\n");
    assert_eq!(doc.blocks[0].name, "ul");
}

#[test]
fn fenced_code_block_with_language() {
    let doc = import_str("```rust\nfn main() {}\n```\n");
    assert_eq!(doc.blocks.len(), 1);
    let code = &doc.blocks[0];
    assert_eq!(code.name, "code");
    assert_eq!(code.prop_str("lang"), Some("rust"));
    assert!(code.plain_text().unwrap().contains("fn main"));
}

#[test]
fn link_imports() {
    let doc = import_str("Visit [example](https://example.com).\n");
    let p = &doc.blocks[0];
    assert_eq!(p.name, "p");
    if let Body::Text(pieces) = &p.body {
        let link = pieces.iter().find_map(|pc| match pc {
            TextPiece::Inline(b) if b.name == "link" => Some(b),
            _ => None,
        });
        let link = link.expect("expected @link inline");
        assert_eq!(link.prop_str("to"), Some("https://example.com"));
        assert_eq!(link.plain_text().unwrap(), "example");
    } else {
        panic!("expected Text body");
    }
}

#[test]
fn inline_code_imports() {
    let doc = import_str("Use `cargo build` to compile.\n");
    let p = &doc.blocks[0];
    if let Body::Text(pieces) = &p.body {
        let code = pieces.iter().find_map(|pc| match pc {
            TextPiece::Inline(b) if b.name == "code" => Some(b),
            _ => None,
        });
        assert!(code.is_some(), "expected @code inline");
    } else {
        panic!("expected Text body");
    }
}

#[test]
fn round_trip_to_html_is_clean() {
    // Sanity check: importing MD then rendering to HTML produces
    // something reasonable. Doesn't assert exact bytes — just that the
    // pipeline survives end-to-end.
    let doc = import_str("# Hello\n\nThis is a *paragraph* with [a link](http://a).\n");
    let html = HtmlExporter::fragment()
        .export(&doc, &stem_core::theme::Theme::default())
        .expect("export");
    assert!(html.contains("<h1>"), "missing <h1>: {}", html);
    assert!(html.contains("Hello"));
    assert!(html.contains("<p>"));
    assert!(html.contains("font-style:italic"));
    assert!(html.contains(r#"<a href="http://a""#));
}

#[test]
fn doc_type_metadata_set_to_document() {
    let doc = import_str("# Hi\n");
    assert_eq!(doc.metadata.get_str("type"), Some("document"));
}
