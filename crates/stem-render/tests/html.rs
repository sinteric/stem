use stem_core::theme::Theme;
use stem_parser::parse;
use stem_render::{HtmlRenderer, Renderer};

fn render(src: &str) -> String {
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "parse errors: {:?}", r.diagnostics);
    let h = HtmlRenderer::fragment();
    h.render(&r.document, &Theme::default()).expect("render")
}

#[test]
fn fragment_starts_with_stem_doc_wrapper() {
    let html = render("[type:document]\nHello world.");
    assert!(html.contains("<div class=\"stem-doc\">"));
    assert!(html.contains("Hello world."));
}

#[test]
fn section_renders_as_section_element() {
    let html = render("[type:document]\nsection(cover)(\n  # Hello\n)");
    assert!(
        html.contains("<section data-id=\"cover\">"),
        "missing section tag: {}",
        html
    );
    assert!(html.contains("<h1>Hello</h1>"));
}

#[test]
fn inline_text_call_applies_color() {
    let html = render(
        "[type:document]\nThis has text(red bits)[color:red] inside.",
    );
    // Expect inline span with red color (#cf222e from default theme)
    assert!(
        html.contains("color:#cf222e"),
        "expected color style, got: {}",
        html
    );
    assert!(html.contains("red bits"));
}

#[test]
fn table_renders_with_header_row() {
    let src = "[type:document]\ntable[border:outer](\n  row(header)(\n    cell(A)\n    cell(B)\n  )\n  row(\n    cell(1)\n    cell(2)\n  )\n)";
    let html = render(src);
    assert!(html.contains("<table"), "no table: {}", html);
    assert!(html.contains("<th"), "no <th>: {}", html);
    assert!(html.contains("<td"), "no <td>: {}", html);
}

#[test]
fn layout_two_column_uses_css_grid() {
    let src = "[type:document]\nlayout(two-column)(\n  col(left)\n  col(right)\n)";
    let html = render(src);
    assert!(
        html.contains("grid-template-columns:1fr 1fr"),
        "missing grid CSS: {}",
        html
    );
    assert!(html.contains("class=\"stem-col\""));
}

#[test]
fn html_escaping_prevents_xss() {
    // Note: `alert(1)` is itself a syntactically valid function call in
    // Stem (`alert` is an ident, `(1)` is an arg group). The parser will
    // turn it into an unknown-function call which the renderer wraps in a
    // <span data-stem="alert">. The security property to verify is that
    // the *surrounding* HTML tags are escaped to prevent script injection
    // — which is what html_text guards against on all rendered text.
    let html = render("[type:document]\nHere is <script>data</script>.");
    assert!(
        !html.contains("<script>"),
        "raw <script> tag leaked through: {}",
        html
    );
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("&lt;/script&gt;"));
}

#[test]
fn list_items_preserve_text_after_marker() {
    let src = "[type:document]\nsection(body)(\n  - Format fragmentation\n  - Hard to generate\n)";
    let html = render(src);
    assert!(
        html.contains("<li>Format fragmentation</li>"),
        "list item text chopped: {}",
        html
    );
    assert!(
        html.contains("<li>Hard to generate</li>"),
        "list item text chopped: {}",
        html
    );
}

#[test]
fn marker_section_does_not_render_id_as_body() {
    let html = render("[type:document]\nsection(toc)\nsection(body)(\n  hello\n)");
    // toc section should NOT contain the literal "toc" as text content
    assert!(
        !html.contains("<p>toc</p>"),
        "section(toc) leaked its id into a paragraph: {}",
        html
    );
    assert!(html.contains("class=\"stem-toc\""));
}

#[test]
fn full_document_includes_doctype_and_locale() {
    let r = parse("[type:document, locale:ko-KR, title:\"제목\"]\nHello.");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let h = HtmlRenderer::new();
    let html = h.render(&r.document, &Theme::default()).unwrap();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("lang=\"ko-KR\""));
    assert!(html.contains("<title>제목</title>"));
}
