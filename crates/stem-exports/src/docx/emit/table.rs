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
use super::ctx::EmitCtx;
use super::{field, run};

/// Content area on letter paper with 1" margins everywhere: 12240
/// dxa page width minus 2×1440 dxa margins.
const CONTENT_WIDTH_DXA: u32 = 12240 - 2 * 1440;

const STRIPE_FILL: &str = "F2F2F2";

/// Emit a `<w:tbl>` from a `table` block plus a trailing caption
/// paragraph if `caption:` is set. `ctx` carries the table SEQ
/// counter so the cached field result on the caption matches
/// document order.
///
/// Property-driven attributes (all reference-matching):
/// - `border: all | outer | none` → `<w:tblBorders>`
/// - `stripe: true` → alternating data-row fill F2F2F2
/// - `indent: Npt | Nin | ...` → `<w:tblInd>` (table left indent)
/// - `widths: "a,b,c,d"` → per-column `<w:gridCol w:w>` values
///   in dxa; falls back to auto-distributed content width.
pub fn render_table(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    let border_mode = match b.prop_str("border").unwrap_or("none") {
        "all" => BorderMode::All,
        "outer" => BorderMode::Outer,
        _ => BorderMode::None,
    };
    let stripe = b.prop_str("stripe").map(|v| v == "true").unwrap_or(false);
    let indent_dxa = b
        .prop_str("indent")
        .and_then(super::paragraph::parse_dxa);

    // Table-level defaults that cascade into rows when the row
    // doesn't set its own value. Mirrors the existing row→cell
    // cascade for bg/color so authors can set the property once at
    // the layer where it makes sense (the table) rather than
    // repeating it on every row.
    let table_row_height = b.prop_str("row-height").and_then(super::paragraph::parse_dxa);
    let table_row_height_rule = b.prop_str("row-height-rule").and_then(parse_height_rule);

    let rows: &[Block] = match &b.body {
        Body::Children(c) => c,
        _ => &[],
    };

    let grid_cols = compute_grid_cols(rows);
    if grid_cols == 0 {
        emit_caption(b, ctx, x);
        return;
    }

    // Per-column widths come from `[widths:"a,b,c,…"]` on the
    // table; missing entries auto-distribute the remaining content
    // width.
    let col_widths = resolve_col_widths(b.prop_str("widths"), grid_cols);

    x.elem("w:tbl", &[], |x| {
        render_tbl_pr(x, border_mode, indent_dxa);
        render_tbl_grid(x, &col_widths);
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
                &col_widths,
                table_row_height,
                table_row_height_rule,
                &mut pending,
                ctx,
                x,
            );
            if !is_header {
                data_row_idx += 1;
            }
        }
    });

    emit_caption(b, ctx, x);
}

/// Parse the `height-rule` / `row-height-rule` property to OOXML's
/// `w:hRule`. Returns `None` for unrecognized values so the caller
/// can keep the default ("atLeast", which OOXML implies when the
/// attribute is omitted).
fn parse_height_rule(s: &str) -> Option<&'static str> {
    match s {
        "atLeast" | "at-least" | "min" => Some("atLeast"),
        "exact" => Some("exact"),
        "auto" => Some("auto"),
        _ => None,
    }
}

fn resolve_col_widths(spec: Option<&str>, grid_cols: usize) -> Vec<u32> {
    let default_each = CONTENT_WIDTH_DXA / grid_cols as u32;
    let mut out: Vec<u32> = (0..grid_cols).map(|_| default_each).collect();
    let Some(spec) = spec else {
        return out;
    };
    for (i, raw) in spec.split(',').enumerate() {
        if i >= grid_cols {
            break;
        }
        if let Some(dxa) = parse_column_width(raw.trim()) {
            out[i] = dxa;
        }
    }
    out
}

/// Per-column width parser. Unlike `parse_dxa` (which treats bare
/// numbers as points), table grid widths are conventionally
/// expressed in raw dxa (Word's `<w:gridCol w:w="…"/>` unit), so a
/// bare number is interpreted as already-dxa. Explicit units
/// (`pt`/`in`/`cm`/`mm`/`px`) convert through `parse_dxa`.
fn parse_column_width(s: &str) -> Option<u32> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return t.parse::<f64>().ok().map(|v| v.round() as u32);
    }
    super::paragraph::parse_dxa(t)
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

fn render_tbl_pr(x: &mut XmlBuf, mode: BorderMode, indent_dxa: Option<u32>) {
    x.elem("w:tblPr", &[], |x| {
        x.empty(
            "w:tblW",
            &[("w:w", "0"), ("w:type", "auto")],
        );
        if let Some(ind) = indent_dxa {
            let s = ind.to_string();
            x.empty("w:tblInd", &[("w:w", &s), ("w:type", "dxa")]);
        }
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

fn render_tbl_grid(x: &mut XmlBuf, col_widths: &[u32]) {
    x.elem("w:tblGrid", &[], |x| {
        for w in col_widths {
            let s = w.to_string();
            x.empty("w:gridCol", &[("w:w", &s)]);
        }
    });
}

fn render_tr(
    row: &Block,
    is_header: bool,
    stripe: bool,
    data_row_idx: usize,
    col_widths: &[u32],
    table_row_height: Option<u32>,
    table_row_height_rule: Option<&'static str>,
    pending: &mut Vec<RowspanCarry>,
    ctx: &mut EmitCtx,
    x: &mut XmlBuf,
) {
    let row_bg = row.prop_str("bg").and_then(normalize_hex);
    let row_color = row.prop_str("color").and_then(normalize_hex);
    // Row height cascade: explicit `row[height:..]` > table-level
    // `table[row-height:..]` default. Same for the rule. `height:Npt`
    // (or any unit `parse_dxa` accepts) sets `<w:trHeight w:val=N/>`;
    // `height-rule:` picks OOXML's `w:hRule` — `atLeast` (default),
    // `exact`, or `auto`.
    let row_height = row
        .prop_str("height")
        .and_then(super::paragraph::parse_dxa)
        .or(table_row_height);
    let row_height_rule = row
        .prop_str("height-rule")
        .and_then(parse_height_rule)
        .or(table_row_height_rule);
    x.elem("w:tr", &[], |x| {
        if is_header || row_height.is_some() {
            x.elem("w:trPr", &[], |x| {
                if let Some(h) = row_height {
                    let h_s = h.to_string();
                    let mut attrs: Vec<(&str, &str)> = vec![("w:val", h_s.as_str())];
                    if let Some(rule) = row_height_rule {
                        attrs.push(("w:hRule", rule));
                    }
                    x.empty("w:trHeight", &attrs);
                }
                if is_header {
                    // Repeat header row when a table spans page breaks.
                    x.empty("w:tblHeader", &[]);
                }
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
                let w = sum_widths(col_widths, grid_col, carry.grid_span);
                render_continuation_tc(carry.grid_span, w, x);
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

            // Cell bg cascade: explicit cell bg > row bg > stripe.
            let force_bg = cell
                .prop_str("bg")
                .and_then(normalize_hex)
                .or_else(|| row_bg.clone())
                .or_else(|| {
                    if !is_header && stripe && data_row_idx % 2 == 1 {
                        Some(STRIPE_FILL.to_string())
                    } else {
                        None
                    }
                });

            // Cell color cascade: explicit cell color > row color.
            let cell_color = cell
                .prop_str("color")
                .and_then(normalize_hex)
                .or_else(|| row_color.clone());

            let w = sum_widths(col_widths, grid_col, colspan);
            render_tc(
                cell,
                is_header,
                colspan,
                rowspan,
                w,
                force_bg,
                cell_color,
                ctx,
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

fn sum_widths(widths: &[u32], start: usize, span: usize) -> u32 {
    widths
        .iter()
        .skip(start)
        .take(span)
        .copied()
        .sum::<u32>()
}

fn render_continuation_tc(grid_span: usize, tcw_dxa: u32, x: &mut XmlBuf) {
    let w = tcw_dxa.to_string();
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
    tcw_dxa: u32,
    force_bg: Option<String>,
    color: Option<String>,
    ctx: &mut EmitCtx,
    x: &mut XmlBuf,
) {
    let w = tcw_dxa.to_string();
    let span_s = colspan.to_string();
    let bg = force_bg;
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
        emit_cell_paragraphs(cell, is_header, color.as_deref(), ctx, x);
    });
}

fn emit_cell_paragraphs(
    cell: &Block,
    is_header: bool,
    color: Option<&str>,
    ctx: &mut EmitCtx,
    x: &mut XmlBuf,
) {
    let cell_align = cell.prop_str("align").and_then(map_jc);
    let base_rpr = super::run::RPr {
        bold: is_header,
        color: color.map(|s| s.to_string()),
        ..Default::default()
    };

    let mut emitted = 0usize;
    match &cell.body {
        Body::None => {}
        Body::Text(_) => {
            emit_cell_paragraph(cell, cell_align, &base_rpr, ctx, x);
            emitted += 1;
        }
        Body::Children(children) => {
            for child in children {
                emit_cell_paragraph(child, cell_align, &base_rpr, ctx, x);
                emitted += 1;
            }
        }
    }
    if emitted == 0 {
        // Word requires every cell to contain at least one paragraph.
        x.empty("w:p", &[]);
    }
}

fn emit_cell_paragraph(
    b: &Block,
    cell_align: Option<&'static str>,
    base: &super::run::RPr,
    ctx: &mut EmitCtx,
    x: &mut XmlBuf,
) {
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
        run::render_body_with(b, ctx, x, base);
    });
}

fn emit_caption(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    let Some(text) = b.prop_str("caption") else {
        return;
    };
    ctx.table_caption_seq += 1;
    let seq_n = ctx.table_caption_seq;
    // Anchor so LoT PAGEREF (task 12) resolves.
    let bookmark = format!("_Toc_table_{seq_n}");
    let bm_id = ctx.alloc_bookmark_id();
    let bm_id_s = bm_id.to_string();
    x.elem("w:p", &[], |x| {
        x.elem("w:pPr", &[], |x| {
            x.empty("w:pStyle", &[("w:val", "Caption")]);
        });
        x.empty(
            "w:bookmarkStart",
            &[("w:id", &bm_id_s), ("w:name", &bookmark)],
        );
        // "Table " label run.
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], "Table ", true);
        });
        // SEQ Table field — Word increments this on F9. Cached
        // result matches our pre-computed counter so the caption
        // reads right on first open.
        field::render_seq("Table", seq_n, x);
        // Separator + user-provided caption text.
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], &format!(". {text}"), true);
        });
        x.empty("w:bookmarkEnd", &[("w:id", &bm_id_s)]);
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
        let mut ctx = EmitCtx::new(None, 1);
        let mut x = XmlBuf::new();
        for b in &r.document.blocks {
            if b.name == "table" {
                render_table(b, &mut ctx, &mut x);
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
        // Caption text is wrapped in "Table N. <text>" with a SEQ
        // field for the number.
        assert!(s.contains("Table "));
        assert!(s.contains(". Hi"));
        assert!(s.contains(r#"w:instr=" SEQ Table \* ARABIC ""#));
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
    fn row_height_emits_tr_height_with_default_rule() {
        // `row[height:24pt]` → `<w:trHeight w:val="480"/>` (no
        // hRule attribute = atLeast, OOXML's default).
        let s = render(
            r#"table{
  row[height:24pt]{ cell(tall) }
}"#,
        );
        assert!(
            s.contains(r#"<w:trHeight w:val="480"/>"#),
            "expected w:trHeight w:val=480 (24pt = 480 dxa): {s}"
        );
    }

    #[test]
    fn row_height_rule_exact_propagates_to_tr_height() {
        let s = render(
            r#"table{
  row[height:30pt, height-rule:exact]{ cell(fixed) }
}"#,
        );
        assert!(
            s.contains(r#"<w:trHeight w:val="600" w:hRule="exact"/>"#),
            "expected exact rule on trHeight: {s}"
        );
    }

    #[test]
    fn row_height_coexists_with_header_marker() {
        // Both `height:` and `kind:header` go into the same <w:trPr>.
        let s = render(
            r#"table{
  row[kind:header, height:22pt]{ cell(H) }
}"#,
        );
        assert!(s.contains(r#"<w:trHeight w:val="440"/>"#), "{s}");
        assert!(s.contains("<w:tblHeader/>"), "{s}");
    }

    #[test]
    fn table_row_height_cascades_into_unset_rows() {
        // `table[row-height:20pt]` applies to every row that doesn't
        // set its own `height:`. The 24pt header overrides; the bare
        // row inherits the table default.
        let s = render(
            r#"table[row-height:20pt]{
  row[kind:header, height:24pt]{ cell(H) }
  row{ cell(A) }
  row{ cell(B) }
}"#,
        );
        assert!(s.contains(r#"<w:trHeight w:val="480"/>"#), "header keeps its own 24pt: {s}");
        // Two data rows each pick up 400 dxa (= 20pt).
        let inherited = s.matches(r#"<w:trHeight w:val="400"/>"#).count();
        assert_eq!(inherited, 2, "expected 2 rows to inherit 20pt: {s}");
    }

    #[test]
    fn table_row_height_rule_cascades_alongside_height() {
        let s = render(
            r#"table[row-height:18pt, row-height-rule:exact]{
  row{ cell(a) }
}"#,
        );
        assert!(
            s.contains(r#"<w:trHeight w:val="360" w:hRule="exact"/>"#),
            "expected cascaded exact rule: {s}"
        );
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
    fn caption_emits_below_table_with_caption_style_and_seq_field() {
        let s = render(
            r#"table[caption:"Hello"]{
  row{ cell(A) }
}"#,
        );
        let tbl_pos = s.find("<w:tbl>").unwrap();
        let cap_pos = s.find(r#"<w:pStyle w:val="Caption"/>"#).unwrap();
        assert!(tbl_pos < cap_pos, "caption should come after tbl");
        // Caption text format: "Table N. <text>".
        assert!(s.contains(". Hello"));
        assert!(s.contains(r#"w:instr=" SEQ Table \* ARABIC ""#));
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
