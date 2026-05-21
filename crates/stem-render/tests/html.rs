use stem_core::theme::Theme;
use stem_parser::parse;
use stem_render::HtmlRenderer;

fn render(src: &str) -> String {
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "parse errors: {:?}", r.diagnostics);
    let h = HtmlRenderer::fragment();
    h.render(&r.document, &Theme::default()).expect("render")
}

#[test]
fn empty_document_renders_wrapper() {
    let html = render("");
    assert!(html.contains("<div class=\"stem-doc\">"));
    assert!(html.contains("</div>"));
}

#[test]
fn heading_renders() {
    let html = render("h1(Hello)");
    assert!(html.contains("<h1>Hello</h1>"), "got: {}", html);
}

#[test]
fn paragraph_with_inline_styling() {
    let html = render("p(The @text[color:red](critical) issue.)");
    assert!(html.contains("color:#cf222e"), "missing color style: {}", html);
    assert!(html.contains("critical"));
    assert!(html.contains("The "));
    assert!(html.contains(" issue."));
}

#[test]
fn section_with_id_renders_as_section_tag() {
    let html = render("section[id:cover]{ h1(Hi) }");
    assert!(html.contains("<section data-id=\"cover\">"));
    assert!(html.contains("<h1>Hi</h1>"));
}

#[test]
fn marker_section_toc_renders_nav() {
    let html = render("section[id:toc]");
    assert!(html.contains("class=\"stem-toc\""));
}

#[test]
fn list_items_render_correctly() {
    let html = render("ol[style:1.]{ li(First) li(Second) }");
    assert!(html.contains("<ol"));
    assert!(html.contains("data-style=\"1.\""));
    assert!(html.contains("<li>First</li>"));
    assert!(html.contains("<li>Second</li>"));
}

#[test]
fn nested_list_renders() {
    let html = render("ul{ li(Top) li{ p(Nested paragraph) } }");
    assert!(html.contains("<ul"));
    assert!(html.contains("<li>Top</li>"));
    assert!(html.contains("<p>Nested paragraph</p>"));
}

#[test]
fn layout_two_column_uses_css_grid() {
    let html = render("layout[kind:two-column]{ col{ h3(L) } col{ h3(R) } }");
    assert!(
        html.contains("grid-template-columns:1fr 1fr"),
        "missing grid CSS: {}",
        html
    );
    assert!(html.contains("class=\"stem-col\""));
}

#[test]
fn table_renders_with_header_row_and_colspan() {
    let src = r#"table[border:outer]{
  row[kind:header]{ cell(A) cell(B) cell[colspan:2](C) }
  row{ cell(1) cell(2) cell(3) cell[bg:yellow](4) }
}"#;
    let html = render(src);
    assert!(html.contains("<table"));
    assert!(html.contains("<th"), "no <th>: {}", html);
    assert!(html.contains("colspan=\"2\""));
    assert!(html.contains("background:#ffd33d"));
}

#[test]
fn html_escapes_dangerous_chars() {
    let html = render("p(some <script>data</script>.)");
    assert!(!html.contains("<script>"), "raw <script> leaked: {}", html);
    assert!(html.contains("&lt;script&gt;"));
}

#[test]
fn unicode_escape_renders_codepoint() {
    let html = render(r#"p("zero-width: \u{200B} between")"#);
    assert!(html.contains("\u{200B}"));
}

#[test]
fn quoted_body_preserves_special_chars() {
    let html = render(r#"cell[colspan:2]("=SUM(B2:B4)")"#);
    // cell would be inside a row normally, but renders standalone here
    // as a fallback. The key is the formula text survives.
    assert!(html.contains("=SUM(B2:B4)"));
}

#[test]
fn footnote_renders_as_sup() {
    let html = render("p(See @footnote(Smith 2024) for details.)");
    assert!(html.contains("<sup class=\"stem-footnote\""));
    assert!(html.contains("Smith 2024"));
}

#[test]
fn sheet_renders_grid_with_fill_and_cascade() {
    let src = r#"[type:sheet, name:"Q4"]
sheet[id:Q4-2026]{
  col[at:B, fmt:currency]
  row[at:1, weight:bold, bg:gray]
  fill[at:A1]("
    Item, Revenue, Margin
    Widget, 42000, 0.35
    Total, 80500, 0.40
  ")
  cell[at:C3, bg:yellow]
}"#;
    let html = render(src);

    // The grid header row labels (A, B, C) and row numbers (1, 2, 3)
    for letter in ["A", "B", "C"] {
        assert!(
            html.contains(&format!(">{}</th>", letter)),
            "missing column header {}: {}",
            letter,
            html
        );
    }
    // Cell values — column B has fmt:currency so 42000 becomes $42,000.00
    assert!(html.contains("Item"));
    assert!(html.contains("Widget"));
    assert!(html.contains("$42,000.00"), "currency format missed: {}", html);
    assert!(html.contains("Total"));
    // Cascade: row 1 gets bold + gray bg via row[at:1] rule
    assert!(html.contains("font-weight:700"), "row-bold missed: {}", html);
    // Cascade: B column gets currency format (rendered as data-fmt)
    assert!(
        html.contains("data-fmt=\"currency\""),
        "column fmt missed: {}",
        html
    );
    // C3 override: yellow background (theme color → hex)
    assert!(
        html.contains("background:#ffd33d"),
        "C3 override missed: {}",
        html
    );
}

#[test]
fn sheet_formula_via_at_inline_element() {
    // Cells declare formulas with @formula(...) — NOT with leading =.
    let src = r#"[type:sheet]
sheet[id:demo]{
  col[at:B, fmt:currency]
  cell[at:A1](10)
  cell[at:A2](20)
  cell[at:A3](30)
  cell[at:B1](@formula("SUM(A1:A3)"))
}"#;
    let html = render(src);
    // SUM(10, 20, 30) = 60 → currency format → $60.00
    assert!(html.contains("$60.00"), "missing evaluated value: {}", html);
}

#[test]
fn sheet_formula_with_leading_equals_surfaces_error() {
    let src = r#"[type:sheet]
sheet[id:demo]{
  cell[at:A1](@formula("=SUM(B2:B6)"))
}"#;
    let html = render(src);
    // The renderer surfaces the embed's typed error in the cell display.
    assert!(
        html.contains("#ERROR"),
        "expected formula error in cell display: {}",
        html
    );
}

#[test]
fn sheet_with_no_cells_shows_empty_message() {
    let html = render("[type:sheet]\nsheet[id:empty]{}");
    assert!(html.contains("(empty sheet)"));
}

#[test]
fn full_document_renders_doctype_and_locale() {
    let r = parse(r#"[type:document, locale:ko-KR, title:"제목"]
section{ h1(Hello) p(World.) }"#);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let h = HtmlRenderer::new();
    let html = h.render(&r.document, &Theme::default()).unwrap();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("lang=\"ko-KR\""));
    assert!(html.contains("<title>제목</title>"));
    assert!(html.contains("<h1>Hello</h1>"));
}
