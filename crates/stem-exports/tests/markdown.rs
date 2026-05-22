#![cfg(feature = "markdown")]

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::MarkdownExporter;
use stem_imports::markdown::import_str;

fn md(stem_src_after_import: &str) -> String {
    let doc = import_str(stem_src_after_import);
    MarkdownExporter::new()
        .export(&doc, &Theme::default())
        .expect("export")
}

#[test]
fn heading_exports() {
    let out = md("# Hello\n");
    assert!(out.contains("# Hello"), "got: {:?}", out);
}

#[test]
fn multiple_heading_levels_export() {
    let out = md("# H1\n\n## H2\n\n### H3\n");
    assert!(out.contains("# H1"));
    assert!(out.contains("## H2"));
    assert!(out.contains("### H3"));
}

#[test]
fn paragraph_with_emphasis_round_trips() {
    let out = md("This is *italic* and **bold** text.\n");
    assert!(out.contains("*italic*"), "got: {:?}", out);
    assert!(out.contains("**bold**"), "got: {:?}", out);
}

#[test]
fn ordered_list_round_trips() {
    let out = md("1. First\n2. Second\n3. Third\n");
    for (i, label) in ["1. First", "2. Second", "3. Third"].iter().enumerate() {
        assert!(out.contains(label), "missing {} at {}: {:?}", label, i, out);
    }
}

#[test]
fn unordered_list_round_trips() {
    let out = md("- apple\n- banana\n");
    assert!(out.contains("- apple"));
    assert!(out.contains("- banana"));
}

#[test]
fn fenced_code_round_trips() {
    let out = md("```rust\nfn main() {}\n```\n");
    assert!(out.contains("```rust"), "got: {:?}", out);
    assert!(out.contains("fn main"));
    assert!(out.contains("```"));
}

#[test]
fn link_round_trips() {
    let out = md("Visit [example](https://example.com).\n");
    assert!(out.contains("[example](https://example.com)"), "got: {:?}", out);
}

#[test]
fn inline_code_round_trips() {
    let out = md("Use `cargo build` to compile.\n");
    assert!(out.contains("`cargo build`"), "got: {:?}", out);
}

#[test]
fn blockquote_exports_with_prefix() {
    let out = md("> Quoted text here.\n");
    assert!(out.contains("> Quoted"), "got: {:?}", out);
}

#[test]
fn unknown_block_emits_stem_fence() {
    use stem_core::ast::{Block, Body, Document, Metadata};
    use stem_core::span::Span;
    let doc = Document {
        metadata: Metadata::default(),
        blocks: vec![Block {
            name: "mystery".into(),
            name_span: Span::default(),
            properties: vec![],
            body: Body::None,
            inline_form: false,
            span: Span::default(),
        }],
    };
    let out = MarkdownExporter::new()
        .export(&doc, &Theme::default())
        .unwrap();
    assert!(out.contains("```stem"), "expected stem fence, got: {:?}", out);
    assert!(out.contains("mystery"));
}
