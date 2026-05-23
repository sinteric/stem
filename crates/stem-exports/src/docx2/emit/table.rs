//! Table emission.
//!
//! Stem source:
//!
//!   `table[border:all, stripe:true, caption:"…"]{`
//!       `row[kind:header]{ cell(A) cell(B) }`
//!       `row{ cell[colspan:2](spans) }`
//!       `row{ cell[rowspan:2](tall) cell(x) }`
//!       `row{ cell(y) }`
//!   `}`
//!
//! OOXML target shape:
//!
//!   `<w:tbl>`
//!       `<w:tblPr> tblW/Borders/Layout/Look </w:tblPr>`
//!       `<w:tblGrid> <w:gridCol w:w=…/> × cols </w:tblGrid>`
//!       `<w:tr> <w:tc>… × cells </w:tr> × rows`
//!   `</w:tbl>`
//!
//! Schema order inside `<w:tcPr>` (the slot docx-rs got wrong):
//!   cnfStyle → tcW → gridSpan → hMerge → vMerge → tcBorders →
//!   shd → noWrap → tcMar → textDirection → tcFitText → vAlign →
//!   hideMark

use stem_core::ast::{Block, Body};

use super::super::xml::XmlBuf;
use super::run;

/// Content area on letter paper with 1" margins everywhere: 12240
/// dxa page width minus 2×1440 dxa margins.
const CONTENT_WIDTH_DXA: u32 = 12240 - 2 * 1440;

const STRIPE_FILL: &str = "F2F2F2";

/// Emit a `<w:tbl>` from a `table` block plus a trailing caption
/// paragraph if `caption:` is set.
pub fn render_table(b: &Block, x: &mut XmlBuf) {
    let border_mode = match b.prop_str("border").unwrap_or("none") {
        "all" => BorderMode::All,
        "outer" => BorderMode::Outer,
        _ => BorderMode::None,
    };
    let stripe = b.prop_str("stripe").map(|v| v == "true").unwrap_or(false);

    let rows: &[Block] = match &b.body {
        Body::Children(c) => c,
        _ => &[],
    };

    // Count grid columns from the first source row, accounting for
    // colspan. The same column count applies to every row by Word's
    // table model.
    let grid_cols = compute_grid_cols(rows);
    if grid_cols == 0 {
        // Empty table — emit just the caption (if any) so caption
        // numbering still advances on a future SEQ pass.
        emit_caption(b, x);
        return;
    }
    let col_w_dxa = CONTENT_WIDTH_DXA / grid_cols as u32;

    x.elem("w:tbl", &[], |x| {
        render_tbl_pr(x, border_mode);
        render_tbl_grid(x, grid_cols, col_w_dxa);
        // Rowspan continuation: for each grid column, track remaining
        // continuation rows + their colspan so we can synthesize
        // `<w:vMerge/>` continuation cells in subsequent rows.
        let mut pending: Vec<RowspanCarry> = Vec::new();
        let mut data_row_idx: usize = 0;
        for row in rows {
            if row.name != "row" {
                continue;
            }
            let is_header = row.prop_str("kind") == Some("header");
            render_tr(
                row,
                is_header,
                stripe,
                data_row_idx,
                col_w_dxa,
                &mut pending,
                x,
            );
            if !is_header {
                data_row_idx += 1;
            }
        }
    });

    emit_caption(b, x);
}

#[derive(Clone, Copy)]
enum BorderMode {
    None,
    Outer,
    All,
}

#[derive(Clone, Copy, Default)]
struct RowspanCarry {
    /// Remaining rows that still need a continuation cell at this
    /// grid slot (after the current row's emission).
    remaining: u32,
    /// Grid columns the carry spans — the continuation cell needs
    /// to repeat the same gridSpan to keep the row width right.
    grid_span: usize,
}

fn compute_grid_cols(rows: &[Block]) -> usize {
    rows.iter()
        .filter(|r| r.name == "row")
        .map(|row| {
            if let Body::Children(cells) = &row.body {
                cells
                    .iter()
                    .filter(|c| c.name == "cell")
                    .map(|c| {
                        c.prop_str("colspan")
                            .and_then(|s| s.parse::<usize>().ok())
                            .filter(|&n| n > 0)
                            .unwrap_or(1)
                    })
                    .sum::<usize>()
            } else {
                0
            }
        })
        .max()
        .unwrap_or(0)
}

fn render_tbl_pr(x: &mut XmlBuf, mode: BorderMode) {
    x.elem("w:tblPr", &[], |x| {
        x.empty(
            "w:tblW",
            &[("w:w", "0"), ("w:type", "auto")],
        );
        render_tbl_borders(x, mode);
        // Fixed layout keeps explicit column widths honored.
        x.empty("w:tblLayout", &[("w:type", "fixed")]);
        x.empty(
            "w:tblLook",
            &[
                ("w:val", "04A0"),
                ("w:firstRow", "1"),
                ("w:lastRow", "0"),
                ("w:firstColumn", "1"),
                ("w:lastColumn", "0"),
                ("w:noHBand", "0"),
                ("w:noVBand", "1"),
            ],
        );
    });
}

fn render_tbl_borders(x: &mut XmlBuf, mode: BorderMode) {
    match mode {
        BorderMode::None => {
            // `nil` borders on every position so Word doesn't draw
            // the auto thin lines.
            x.elem("w:tblBorders", &[], |x| {
                for pos in ["top", "left", "bottom", "right", "insideH", "insideV"] {
                    x.empty(
                        &format!("w:{pos}"),
                        &[("w:val", "nil"), ("w:sz", "0"), ("w:space", "0"), ("w:color", "auto")],
                    );
                }
            });
        }
        BorderMode::Outer => {
            x.elem("w:tblBorders", &[], |x| {
                for pos in ["top", "left", "bottom", "right"] {
                    x.empty(
                        &format!("w:{pos}"),
                        &[("w:val", "single"), ("w:sz", "4"), ("w:space", "0"), ("w:color", "auto")],
                    );
                }
                for pos in ["insideH", "insideV"] {
                    x.empty(
                        &format!("w:{pos}"),
                        &[("w:val", "nil"), ("w:sz", "0"), ("w:space", "0"), ("w:color", "auto")],
                    );
                }
            });
        }
        BorderMode::All => {
            x.elem("w:tblBorders", &[], |x| {
                for pos in ["top", "left", "bottom", "right", "insideH", "insideV"] {
                    x.empty(
                        &format!("w:{pos}"),
                        &[("w:val", "single"), ("w:sz", "4"), ("w:space", "0"), ("w:color", "auto")],
                    );
                }
            });
        }
    }
}

fn render_tbl_grid(x: &mut XmlBuf, cols: usize, col_w_dxa: u32) {
    x.elem("w:tblGrid", &[], |x| {
        let w = col_w_dxa.to_string();
        for _ in 0..cols {
            x.empty("w:gridCol", &[("w:w", &w)]);
        }
    });
}

fn render_tr(
    row: &Block,
    is_header: bool,
    stripe: bool,
    data_row_idx: usize,
    col_w_dxa: u32,
    pending: &mut Vec<RowspanCarry>,
    x: &mut XmlBuf,
) {
    x.elem("w:tr", &[], |x| {
        if is_header {
            x.elem("w:trPr", &[], |x| {
                // Repeat header row when a table spans page breaks.
                x.empty("w:tblHeader", &[]);
            });
        }

        let cells: &[Block] = match &row.body {
            Body::Children(c) => c,
            _ => &[],
        };
        let mut source_iter = cells.iter().filter(|c| c.name == "cell");
        let mut grid_col: usize = 0;

        loop {
            // Continuation slot from a prior row's vMerge.
            if grid_col < pending.len() && pending[grid_col].remaining > 0 {
                let carry = pending[grid_col];
                render_continuation_tc(carry.grid_span, col_w_dxa, x);
                for i in 0..carry.grid_span {
                    if let Some(slot) = pending.get_mut(grid_col + i) {
                        slot.remaining = slot.remaining.saturating_sub(1);
                    }
                }
                grid_col += carry.grid_span;
                continue;
            }

            let Some(cell) = source_iter.next() else {
                break;
            };
            let colspan: usize = cell
                .prop_str("colspan")
                .and_then(|s| s.parse().ok())
                .filter(|&n: &usize| n > 0)
                .unwrap_or(1);
            let rowspan: u32 = cell
                .prop_str("rowspan")
                .and_then(|s| s.parse().ok())
                .filter(|&n| n > 0)
                .unwrap_or(1);

            let force_bg = if !is_header && stripe && data_row_idx % 2 == 1 {
                Some(STRIPE_FILL.to_string())
            } else {
                None
            };

            render_tc(
                cell,
                is_header,
                colspan,
                rowspan,
                col_w_dxa,
                force_bg,
                x,
            );

            // Track the vMerge state for continuation rows.
            if rowspan > 1 {
                while pending.len() < grid_col + colspan {
                    pending.push(RowspanCarry::default());
                }
                pending[grid_col] = RowspanCarry {
                    remaining: rowspan - 1,
                    grid_span: colspan,
                };
                for i in 1..colspan {
                    pending[grid_col + i] = RowspanCarry::default();
                }
            } else {
                while pending.len() < grid_col + colspan {
                    pending.push(RowspanCarry::default());
                }
            }
            grid_col += colspan;
        }

        // Trim trailing exhausted entries.
        while matches!(
            pending.last(),
            Some(RowspanCarry { remaining: 0, .. })
        ) {
            pending.pop();
        }
    });
}

fn render_continuation_tc(grid_span: usize, col_w_dxa: u32, x: &mut XmlBuf) {
    let w = (col_w_dxa * grid_span as u32).to_string();
    let span_s = grid_span.to_string();
    x.elem("w:tc", &[], |x| {
        x.elem("w:tcPr", &[], |x| {
            x.empty("w:tcW", &[("w:w", &w), ("w:type", "dxa")]);
            if grid_span > 1 {
                x.empty("w:gridSpan", &[("w:val", &span_s)]);
            }
            // Empty val => continue.
            x.empty("w:vMerge", &[]);
        });
        x.empty("w:p", &[]);
    });
}

fn render_tc(
    cell: &Block,
    is_header: bool,
    colspan: usize,
    rowspan: u32,
    col_w_dxa: u32,
    force_bg: Option<String>,
    x: &mut XmlBuf,
) {
    let w = (col_w_dxa * colspan as u32).to_string();
    let span_s = colspan.to_string();
    let bg = force_bg.or_else(|| cell.prop_str("bg").and_then(normalize_hex));
    let valign = cell.prop_str("valign").and_then(map_valign);

    x.elem("w:tc", &[], |x| {
        // <w:tcPr> children in canonical order: tcW → gridSpan →
        // vMerge → shd → vAlign.
        x.elem("w:tcPr", &[], |x| {
            x.empty("w:tcW", &[("w:w", &w), ("w:type", "dxa")]);
            if colspan > 1 {
                x.empty("w:gridSpan", &[("w:val", &span_s)]);
            }
            if rowspan > 1 {
                x.empty("w:vMerge", &[("w:val", "restart")]);
            }
            if let Some(fill) = &bg {
                x.empty(
                    "w:shd",
                    &[("w:val", "clear"), ("w:color", "auto"), ("w:fill", fill.as_str())],
                );
            }
            if let Some(va) = valign {
                x.empty("w:vAlign", &[("w:val", va)]);
            }
        });

        // Cell content — at least one paragraph required, even if
        // the cell is "empty".
        emit_cell_paragraphs(cell, is_header, x);
    });
}

fn emit_cell_paragraphs(cell: &Block, is_header: bool, x: &mut XmlBuf) {
    let cell_align = cell.prop_str("align").and_then(map_jc);
    let base_rpr = if is_header {
        super::run::RPr {
            bold: true,
            ..Default::default()
        }
    } else {
        super::run::RPr::default()
    };

    let mut emitted = 0usize;
    match &cell.body {
        Body::None => {}
        Body::Text(_) => {
            emit_cell_paragraph(cell, cell_align, &base_rpr, x);
            emitted += 1;
        }
        Body::Children(children) => {
            for child in children {
                emit_cell_paragraph(child, cell_align, &base_rpr, x);
                emitted += 1;
            }
        }
    }
    if emitted == 0 {
        // Word requires every cell to contain at least one paragraph.
        x.empty("w:p", &[]);
    }
}

fn emit_cell_paragraph(b: &Block, cell_align: Option<&'static str>, base: &super::run::RPr, x: &mut XmlBuf) {
    x.elem("w:p", &[], |x| {
        let child_align = b.prop_str("align").and_then(map_jc);
        let jc = child_align.or(cell_align);
        if jc.is_some() {
            x.elem("w:pPr", &[], |x| {
                if let Some(j) = jc {
                    x.empty("w:jc", &[("w:val", j)]);
                }
            });
        }
        // Reuse the run dispatcher with the inherited rPr (so
        // header cells get bold runs).
        run::render_body_with(b, x, base);
    });
}

fn emit_caption(b: &Block, x: &mut XmlBuf) {
    let Some(text) = b.prop_str("caption") else {
        return;
    };
    x.elem("w:p", &[], |x| {
        x.elem("w:pPr", &[], |x| {
            x.empty("w:pStyle", &[("w:val", "Caption")]);
        });
        // Caption paragraphs are plain text for task 8; task 12
        // wires the SEQ field + bookmark anchor for LoT entries.
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], text, true);
        });
    });
}

fn normalize_hex(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches('#');
    if t.len() == 6 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(t.to_uppercase())
    } else {
        None
    }
}

fn map_jc(s: &str) -> Option<&'static str> {
    match s {
        "left" => Some("left"),
        "right" => Some("right"),
        "center" | "centre" => Some("center"),
        "justify" | "both" => Some("both"),
        _ => None,
    }
}

fn map_valign(s: &str) -> Option<&'static str> {
    match s {
        "top" => Some("top"),
        "middle" | "center" => Some("center"),
        "bottom" => Some("bottom"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use stem_parser::parse;

    use super::*;

    fn render(src: &str) -> String {
        let r = parse(src);
        let mut x = XmlBuf::new();
        for b in &r.document.blocks {
            if b.name == "table" {
                render_table(b, &mut x);
            }
        }
        x.finish()
    }

    #[test]
    fn empty_table_renders_only_caption_if_any() {
        let s = render(r#"table[caption:"Hi"]{}"#);
        // No <w:tbl>, just the caption paragraph.
        assert!(!s.contains("<w:tbl>"));
        assert!(s.contains("<w:pStyle w:val=\"Caption\"/>"));
        assert!(s.contains("Hi"));
    }

    #[test]
    fn simple_2x2_table_emits_tbl_with_grid() {
        let s = render(
            r#"table[border:all]{
  row{ cell(A) cell(B) }
  row{ cell(C) cell(D) }
}"#,
        );
        assert!(s.contains("<w:tbl>"));
        assert!(s.contains("<w:tblBorders>"));
        // 2 grid cols emitted.
        assert_eq!(s.matches("<w:gridCol ").count(), 2);
        // 2 rows × 2 cells = 4 tc.
        assert_eq!(s.matches("<w:tc>").count(), 4);
        assert!(s.contains("A") && s.contains("D"));
    }

    #[test]
    fn header_row_runs_are_bold() {
        let s = render(
            r#"table{
  row[kind:header]{ cell(H1) cell(H2) }
  row{ cell(d1) cell(d2) }
}"#,
        );
        assert!(s.contains("<w:tblHeader/>"));
        // Bold rPr on the header runs.
        assert!(s.contains("<w:b/>"));
    }

    #[test]
    fn colspan_emits_gridSpan_and_widens_tcW() {
        let s = render(
            r#"table{
  row{ cell[colspan:2](spans) cell(c) }
  row{ cell(a) cell(b) cell(c) }
}"#,
        );
        assert!(s.contains(r#"<w:gridSpan w:val="2"/>"#));
        // The widened tcW value should be 2× the per-column width.
        let per_col = CONTENT_WIDTH_DXA / 3;
        let wide = per_col * 2;
        let needle = format!(r#"<w:tcW w:w="{wide}""#);
        assert!(s.contains(&needle), "missing wide tcW {wide}: {s}");
    }

    #[test]
    fn rowspan_emits_vMerge_restart_then_continue() {
        let s = render(
            r#"table{
  row{ cell[rowspan:2](tall) cell(x) }
  row{ cell(y) }
}"#,
        );
        assert!(s.contains(r#"<w:vMerge w:val="restart"/>"#));
        // Continuation cell uses `<w:vMerge/>` with no value.
        assert!(s.contains("<w:vMerge/>"));
    }

    #[test]
    fn stripe_applies_to_alternate_data_rows() {
        let s = render(
            r#"table[stripe:true]{
  row[kind:header]{ cell(H) }
  row{ cell(r1) }
  row{ cell(r2) }
  row{ cell(r3) }
}"#,
        );
        // Stripe fill present at least once.
        assert!(
            s.contains(r#"w:fill="F2F2F2""#),
            "expected stripe fill: {s}"
        );
    }

    #[test]
    fn cell_align_emits_w_jc_on_paragraph() {
        let s = render(
            r#"table{
  row{ cell[align:right](r) }
}"#,
        );
        assert!(s.contains(r#"<w:jc w:val="right"/>"#));
    }

    #[test]
    fn cell_valign_emits_w_vAlign_on_tcPr() {
        let s = render(
            r#"table{
  row{ cell[valign:middle](m) }
}"#,
        );
        assert!(s.contains(r#"<w:vAlign w:val="center"/>"#));
    }

    #[test]
    fn caption_emits_below_table_with_caption_style() {
        let s = render(
            r#"table[caption:"Hello"]{
  row{ cell(A) }
}"#,
        );
        let tbl_pos = s.find("<w:tbl>").unwrap();
        let cap_pos = s.find(r#"<w:pStyle w:val="Caption"/>"#).unwrap();
        assert!(tbl_pos < cap_pos, "caption should come after tbl");
    }

    #[test]
    fn tcPr_child_order_is_canonical() {
        let s = render(
            r##"table{
  row{ cell[colspan:2, rowspan:2, bg:"#cccccc", valign:middle](x) cell(y) }
  row{ cell(z) }
}"##,
        );
        // First tcPr should have tcW < gridSpan < vMerge < shd < vAlign.
        let tcpr = s.find("<w:tcPr>").unwrap();
        let end = s[tcpr..].find("</w:tcPr>").unwrap() + tcpr;
        let block = &s[tcpr..end];
        let w = block.find("<w:tcW").unwrap();
        let gs = block.find("<w:gridSpan").unwrap();
        let vm = block.find("<w:vMerge").unwrap();
        let shd = block.find("<w:shd ").unwrap();
        let va = block.find("<w:vAlign").unwrap();
        assert!(w < gs, "tcW < gridSpan: {block}");
        assert!(gs < vm, "gridSpan < vMerge: {block}");
        assert!(vm < shd, "vMerge < shd: {block}");
        assert!(shd < va, "shd < vAlign: {block}");
    }
}
