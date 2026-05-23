use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::HtmlExporter;
use stem_parser::parse;

fn render(src: &str) -> String {
    let r = parse(src);
    // Filter to error-severity diagnostics — Hint/Info noise (e.g.
    // empty `()` text body suggestion) is informational and doesn't
    // block rendering. Matches the docx test suite's policy.
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, stem_core::Severity::Error))
        .collect();
    assert!(errs.is_empty(), "parse errors: {:?}", errs);
    let h = HtmlExporter::fragment();
    h.export(&r.document, &Theme::default()).expect("export")
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
    assert!(html.contains("<h1 class=\"stem-Heading1\""), "got: {}", html);
    assert!(html.contains(">Hello</h1>"), "got: {}", html);
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
    assert!(html.contains(">Hi</h1>"));
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
fn link_renders_via_per_element_dispatch() {
    let html = render(r#"p(See @link[to:"https://example.com", title:"Example"](here) for more.)"#);
    assert!(
        html.contains(r#"<a href="https://example.com" title="Example">here</a>"#),
        "unexpected link output: {}",
        html
    );
}

#[test]
fn full_document_renders_doctype_and_locale() {
    let r = parse(r#"[type:document, locale:ko-KR, title:"제목"]
section{ h1(Hello) p(World.) }"#);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let h = HtmlExporter::new();
    let html = h.export(&r.document, &Theme::default()).unwrap();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("lang=\"ko-KR\""));
    assert!(html.contains("<title>제목</title>"));
    assert!(html.contains(">Hello</h1>"));
}

#[test]
fn math_latex_renders_mathml() {
    let html = render(r#"p(simple: @math("a^2 + b^2"))"#);
    assert!(html.contains("<math"), "expected <math> tag, got: {}", html);
    assert!(
        html.contains("msup") || html.contains("a") && html.contains("2"),
        "expected MathML structure, got: {}",
        html
    );
}

#[test]
fn math_block_display_uses_block_class() {
    let html = render(r#"p(@math[display:block]("x + y"))"#);
    assert!(
        html.contains("stem-math block"),
        "expected block class, got: {}",
        html
    );
}

#[test]
fn math_mathml_notation_passes_through() {
    let html = render(r#"p(@math[notation:mathml]("<mi>x</mi>"))"#);
    assert!(html.contains("<mi>x</mi>"), "got: {}", html);
}

#[test]
fn math_unsupported_notation_emits_error_span() {
    let html = render(r#"p(@math[notation:asciimath]("a/b"))"#);
    assert!(
        html.contains("stem-math-error"),
        "expected error span for unsupported notation, got: {}",
        html
    );
}

#[test]
fn literal_cell_with_fmt_currency_renders_formatted() {
    let src = r#"[type:sheet]
sheet[id:demo]{
  cell[at:A1, fmt:currency](42000)
}"#;
    let html = render(src);
    assert!(
        html.contains("$42,000.00"),
        "expected currency-formatted literal, got: {}",
        html
    );
}

// -----------------------------------------------------------
// Property-surface parity with docx
// -----------------------------------------------------------

#[test]
fn paragraph_align_and_spacing_props_emit_inline_css() {
    let html = render("p[align:center, before:6pt, after:12pt, line:1.5x, size:14pt](x)");
    assert!(html.contains("text-align:center;"), "{}", html);
    assert!(html.contains("margin-top:6pt;"), "{}", html);
    assert!(html.contains("margin-bottom:12pt;"), "{}", html);
    assert!(html.contains("line-height:1.5;"), "{}", html);
    assert!(html.contains("font-size:14pt;"), "{}", html);
}

#[test]
fn paragraph_border_top_emits_css_rule_and_padding() {
    let html = render("p[border-top:true](sep)");
    assert!(html.contains("border-top:1px solid currentColor;"), "{}", html);
    assert!(html.contains("padding-top:4pt;"), "{}", html);
}

#[test]
fn heading_carries_style_class_and_toc_bookmark() {
    let html = render("h1(Intro)\nh2(Why)");
    assert!(html.contains("<h1 class=\"stem-Heading1\" id=\"_Toc1\""), "{}", html);
    assert!(html.contains("<h2 class=\"stem-Heading2\" id=\"_Toc2\""), "{}", html);
}

#[test]
fn title_block_emits_stem_title_class() {
    let html = render("title(Paper Title)");
    assert!(html.contains("<h1 class=\"stem-Title\""), "{}", html);
    assert!(html.contains(">Paper Title</h1>"), "{}", html);
}

#[test]
fn blockquote_align_lands_on_blockquote() {
    let html = render("blockquote[align:right, size:11pt](quoted)");
    assert!(html.contains("<blockquote"));
    assert!(html.contains("text-align:right;"), "{}", html);
    assert!(html.contains("font-size:11pt;"), "{}", html);
}

#[test]
fn image_defaults_to_centered_figure_and_overrides_via_align() {
    let html_default = render(r#"image[src:"a.png"]"#);
    let fig_default = extract_figure(&html_default);
    assert!(
        fig_default.contains("text-align:center"),
        "default-center: {}", fig_default
    );
    let html_left = render(r#"image[src:"a.png", align:left]"#);
    let fig_left = extract_figure(&html_left);
    assert!(
        fig_left.contains("text-align:left"),
        "override: {}", fig_left
    );
    assert!(
        !fig_left.contains("text-align:center"),
        "default center should not leak onto the override figure: {}", fig_left
    );
}

/// Pull the first `<figure …>` open tag out of the rendered HTML so
/// figure-only assertions don't trip on the style block's own
/// `text-align:center` rules.
fn extract_figure(html: &str) -> &str {
    let start = html.find("<figure").expect("figure present");
    let end_off = html[start..].find('>').expect("figure tag closed") + start + 1;
    &html[start..end_off]
}

#[test]
fn image_w_h_emit_inline_styles_on_img() {
    let html = render(r#"image[src:"a.png", w:6in, h:1.22in, alt:"Logo"]"#);
    assert!(html.contains("width:432pt;"), "{}", html); // 6in = 432pt
    assert!(html.contains("height:87.84pt;") || html.contains("height:88pt;"), "{}", html);
    assert!(html.contains("alt=\"Logo\""), "{}", html);
}

#[test]
fn image_caption_emits_figure_n_prefix_and_bookmark() {
    let html = render(
        r#"image[src:"a.png", caption:"First"]
image[src:"b.png", caption:"Second"]"#,
    );
    assert!(html.contains("<figcaption id=\"_Toc_figure_1\""), "{}", html);
    assert!(html.contains("Figure 1. First"), "{}", html);
    assert!(html.contains("<figcaption id=\"_Toc_figure_2\""), "{}", html);
    assert!(html.contains("Figure 2. Second"), "{}", html);
}

#[test]
fn table_border_class_and_inline_collapse_style() {
    let html = render("table[border:all]{ row{ cell(a) } }");
    assert!(html.contains("class=\"stem-table stem-border-all\""), "{}", html);
    assert!(html.contains("border-collapse:collapse;"), "{}", html);
    assert!(html.contains("border:1px solid currentColor;"), "{}", html);
}

#[test]
fn table_stripe_class_applied_when_stripe_true() {
    let html = render(
        "table[stripe:true]{ row{ cell(a) } row{ cell(b) } row{ cell(c) } }",
    );
    assert!(html.contains("stem-stripe"), "{}", html);
    // Second data row carries the stripe fill (index 1).
    assert!(html.contains("background:#F2F2F2;"), "{}", html);
}

#[test]
fn table_widths_emit_colgroup_with_pt_widths() {
    let html = render(r#"table[widths:"40pt,60pt"]{ row{ cell(a) cell(b) } }"#);
    assert!(html.contains("<colgroup>"));
    assert!(html.contains("width:40pt;"), "{}", html);
    assert!(html.contains("width:60pt;"), "{}", html);
}

#[test]
fn table_indent_lands_on_wrapper_table_style() {
    let html = render("table[indent:18pt]{ row{ cell(x) } }");
    assert!(html.contains("margin-left:18pt;"), "{}", html);
}

#[test]
fn table_caption_emits_caption_with_seq_and_bookmark() {
    let html = render(
        r#"table[caption:"Alpha"]{ row{ cell(x) } }
table[caption:"Beta"]{ row{ cell(y) } }"#,
    );
    assert!(html.contains("<caption class=\"stem-Caption\" id=\"_Toc_table_1\""));
    assert!(html.contains("Table 1. Alpha"), "{}", html);
    assert!(html.contains("Table 2. Beta"), "{}", html);
}

#[test]
fn row_bg_color_cascade_into_cells_unless_cell_overrides() {
    let html = render(
        r##"table{
  row[bg:"#2E74B5", color:"#FFFFFF"]{ cell(plain) cell[bg:"#FF0000"](override) }
}"##,
    );
    // Plain cell picks up row bg + color.
    assert!(html.contains("background:#2E74B5;"), "{}", html);
    assert!(html.contains("color:#FFFFFF;"), "{}", html);
    // Override cell carries the explicit red and not the row blue
    // for background.
    assert!(html.contains("background:#FF0000;"), "{}", html);
}

#[test]
fn cell_colspan_rowspan_align_valign_emit_attrs_and_styles() {
    let html = render(
        "table{ row{ cell[colspan:2, rowspan:3, align:right, valign:middle](merged) } }",
    );
    assert!(html.contains("colspan=\"2\""), "{}", html);
    assert!(html.contains("rowspan=\"3\""), "{}", html);
    assert!(html.contains("text-align:right;"), "{}", html);
    assert!(html.contains("vertical-align:middle;"), "{}", html);
}

#[test]
fn row_height_cascades_from_table_unless_row_overrides() {
    let html = render(
        r#"table[row-height:20pt]{
  row[height:30pt]{ cell(A) }
  row{ cell(B) }
}"#,
    );
    // Explicit row keeps its own 30pt.
    assert!(html.contains("min-height:30pt;"), "{}", html);
    // Bare row inherits table's 20pt.
    assert!(html.contains("min-height:20pt;"), "{}", html);
}

#[test]
fn table_row_height_rule_exact_emits_fixed_height() {
    let html = render(
        r#"table[row-height:18pt, row-height-rule:exact]{
  row{ cell(x) }
}"#,
    );
    assert!(html.contains("height:18pt;"), "{}", html);
    assert!(!html.contains("min-height:18pt;"), "{}", html);
}

#[test]
fn text_size_and_font_emit_css_decls() {
    let html = render(r#"p(@text[size:14pt, font:"Cambria"](styled))"#);
    assert!(html.contains("font-size:14pt;"), "{}", html);
    assert!(html.contains("font-family:\"Cambria\";"), "{}", html);
}

#[test]
fn text_strike_decoration_emits_line_through() {
    let html = render("p(@text[decoration:strike](old))");
    assert!(html.contains("text-decoration:line-through;"), "{}", html);
}

#[test]
fn text_color_hex_passes_through_when_theme_resolution_misses() {
    // The default Theme resolves `red` / `blue` etc to hex; a raw
    // `#C0392B` is not a theme name so should fall through the
    // normalize_hex_color path (uppercased) — but Theme may still
    // resolve it via a hex-parser. Accept either lowercase or
    // uppercase emission.
    let html = render(r##"p(@text[color:"#C0392B"](danger))"##);
    let lower = html.contains("color:#c0392b");
    let upper = html.contains("color:#C0392B");
    assert!(lower || upper, "expected red hex in output: {}", html);
}

#[test]
fn br_inline_emits_html_br() {
    let html = render("p(line1@br()line2)");
    assert!(html.contains("line1<br>line2"), "{}", html);
}

#[test]
fn tab_inline_emits_emsp() {
    let html = render("p(left@tab()right)");
    assert!(html.contains("left&emsp;right"), "{}", html);
}

#[test]
fn page_number_and_total_pages_are_silent_no_ops() {
    let html = render("p(Page @page-number() of @total-pages())");
    // No `data-stem="page-number"` fallback wrapper should leak.
    assert!(!html.contains("data-stem=\"page-number\""), "{}", html);
    assert!(!html.contains("data-stem=\"total-pages\""), "{}", html);
    assert!(html.contains("Page  of "), "{}", html);
}

#[test]
fn header_and_footer_blocks_are_silent_no_ops_in_html_body() {
    let html = render(
        r#"header{ p(chrome top) }
h1(Body)
footer{ p(chrome bottom) }"#,
    );
    // The chrome text must NOT land in the rendered HTML body.
    assert!(!html.contains("chrome top"), "{}", html);
    assert!(!html.contains("chrome bottom"), "{}", html);
    // Body content still renders.
    assert!(html.contains(">Body</h1>"), "{}", html);
}

#[test]
fn style_block_overrides_emit_css_rule_in_head() {
    let r = stem_parser::parse(
        r##"[type:document]
style[id:Heading1, color:"#C0392B", size:20pt]
h1(Hello)"##,
    );
    let h = HtmlExporter::new();
    let html = h.export(&r.document, &Theme::default()).expect("export");
    // The override rule is appended after the default; cascade wins.
    let head_block = html.split("</style>").next().expect("style block");
    let h1_line = head_block
        .lines()
        .find(|l| l.contains(".stem-Heading1"))
        .expect("h1 css line");
    let default = h1_line.find("#2E74B5").expect("default color");
    let overr = h1_line.find("#C0392B").expect("override color");
    assert!(default < overr, "override must follow default: {}", h1_line);
    // And no body paragraph leaked for the style block.
    assert!(!html.contains("data-stem=\"style\""), "{}", html);
}

#[test]
fn caption_style_default_centered_and_overrides_via_style_block() {
    // Default Caption rule centers; the `figcaption`/`table>caption`
    // selectors fold in too.
    let html_default = render(r#"image[src:"a.png", caption:"x"]"#);
    assert!(
        html_default.contains("text-align:center"),
        "default Caption centered: {}", html_default
    );

    let html_override = render(
        r#"style[id:Caption, align:left]
image[src:"a.png", caption:"y"]"#,
    );
    // Override comes after default in the CSS, so cascade leans left.
    let head = html_override.split("</style>").next().expect("style");
    let cap = head
        .lines()
        .find(|l| l.contains(".stem-Caption"))
        .expect("Caption css");
    let center = cap.find("text-align:center").expect("default center");
    let left = cap.find("text-align:left").expect("override left");
    assert!(center < left, "override must follow default: {}", cap);
}

#[test]
fn section_toc_emits_nav_with_per_heading_links() {
    let html = render(
        r#"section[id:toc]
h1(Intro)
h2(Why)
h1(End)"#,
    );
    assert!(html.contains("<nav class=\"stem-toc\""), "{}", html);
    assert!(html.contains("Table of Contents"), "{}", html);
    assert!(html.contains(r##"<a href="#_Toc1">Intro</a>"##), "{}", html);
    assert!(html.contains(r##"<a href="#_Toc2">Why</a>"##), "{}", html);
    assert!(html.contains(r##"<a href="#_Toc3">End</a>"##), "{}", html);
    // Per-level class for indentation.
    assert!(html.contains("class=\"stem-TOC2\""), "{}", html);
}

#[test]
fn section_toc_levels_filters_higher_headings() {
    let html = render(
        r#"section[id:toc, levels:"1-1"]
h1(Top)
h2(Skipped)
h1(Also Top)"#,
    );
    assert!(html.contains(">Top</a>"), "{}", html);
    assert!(html.contains(">Also Top</a>"), "{}", html);
    assert!(!html.contains(">Skipped</a>"), "h2 should be filtered: {}", html);
}

#[test]
fn section_list_of_tables_links_each_caption() {
    let html = render(
        r#"section[id:list-of-tables]
table[caption:"Alpha"]{ row{ cell(a) } }
table[caption:"Beta"]{ row{ cell(b) } }"#,
    );
    assert!(html.contains("<nav class=\"stem-lot\""), "{}", html);
    assert!(html.contains("List of Tables"), "{}", html);
    assert!(html.contains(r##"<a href="#_Toc_table_1">Table 1. Alpha</a>"##));
    assert!(html.contains(r##"<a href="#_Toc_table_2">Table 2. Beta</a>"##));
}

#[test]
fn section_list_of_figures_links_each_caption() {
    let html = render(
        r#"section[id:list-of-figures]
image[src:"a.png", caption:"Pic A"]
image[src:"b.png", caption:"Pic B"]"#,
    );
    assert!(html.contains("<nav class=\"stem-lof\""));
    assert!(html.contains("List of Figures"));
    assert!(html.contains(r##"<a href="#_Toc_figure_1">Figure 1. Pic A</a>"##));
    assert!(html.contains(r##"<a href="#_Toc_figure_2">Figure 2. Pic B</a>"##));
}

#[test]
fn unknown_docx_only_property_does_not_warn_or_break() {
    // `tabs:` has no HTML equivalent; should silently drop without
    // changing the paragraph's other behavior.
    let html = render(r#"p[tabs:"center,right"](left@tab()right)"#);
    assert!(html.contains("<p"), "{}", html);
    assert!(html.contains("left&emsp;right"), "{}", html);
}

#[test]
fn full_document_inlines_style_overrides_into_head() {
    let r = stem_parser::parse(
        r##"[type:document]
style[id:Heading1, color:"#C0392B"]
h1(Hi)"##,
    );
    let html = HtmlExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export");
    assert!(html.contains("<!doctype html>"));
    // Override CSS sits inside the head's <style>.
    let head = html.split("</head>").next().expect("head");
    assert!(head.contains("#C0392B"), "{}", head);
}
