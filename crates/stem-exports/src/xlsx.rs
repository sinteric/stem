//! Stem AST → xlsx via `rust_xlsxwriter`.
//!
//! Maps Stem sheet documents to Excel workbooks: each `sheet` block in
//! a document becomes one worksheet; each `cell[at:X](body)` becomes a
//! cell on the corresponding worksheet. The cook pass has already
//! flattened `fill`/`source`/cascade rules into per-cell blocks before
//! we see them.
//!
//! Cell value resolution:
//! - body is a single `@formula(...)` inline → emit as Excel formula
//! - body is parseable as a number → emit as number
//! - otherwise → emit as string
//!
//! MVP scope: cell values + sheet name. The `fmt:currency`/`percent`/
//! etc. properties don't yet translate to Excel cell formats — values
//! land raw. Future: register a Format per fmt kind and apply.

use rust_xlsxwriter::{Workbook, Worksheet, XlsxError};
use stem_core::ast::{Block, Body, Document, TextPiece};
use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_types::formula;
use thiserror::Error;

#[derive(Default)]
pub struct XlsxExporter;

impl XlsxExporter {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("xlsx: {0}")]
    Xlsx(String),
}

impl From<XlsxError> for Error {
    fn from(value: XlsxError) -> Self {
        Self::Xlsx(value.to_string())
    }
}

impl Exporter for XlsxExporter {
    type Output = Vec<u8>;
    type Error = Error;
    fn export(&self, doc: &Document, _theme: &Theme) -> Result<Vec<u8>, Error> {
        let cooked = stem_parser::cook_document(doc);
        let mut workbook = Workbook::new();

        let mut sheet_count = 0;
        for block in &cooked.blocks {
            if block.name == "sheet" {
                sheet_count += 1;
                let ws = workbook.add_worksheet();
                emit_sheet(ws, block, sheet_count)?;
            }
        }

        // An xlsx file MUST have at least one worksheet. If the document
        // contains no sheet blocks, emit an empty one.
        if sheet_count == 0 {
            let ws = workbook.add_worksheet();
            let _ = ws.set_name("Sheet1");
        }

        Ok(workbook.save_to_buffer()?)
    }
}

fn emit_sheet(ws: &mut Worksheet, sheet: &Block, fallback_index: usize) -> Result<(), XlsxError> {
    // Pick a worksheet name. Excel limits names to 31 chars, no certain
    // special characters; the writer's set_name validates this.
    let raw_name = sheet
        .prop_str("name")
        .or_else(|| sheet.prop_str("id"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Sheet{}", fallback_index));
    let safe_name = sanitize_sheet_name(&raw_name, fallback_index);
    ws.set_name(&safe_name)?;

    if let Body::Children(kids) = &sheet.body {
        for child in kids {
            if child.name != "cell" {
                continue;
            }
            let Some(at) = child.prop_str("at") else { continue };
            let Some((col, row)) = parse_address(at) else { continue };

            // Decide the cell's payload.
            let source = extract_cell_source(child);
            match source {
                CellSource::Formula(text) => {
                    ws.write_formula(row, col, format!("={}", text).as_str())?;
                }
                CellSource::Number(n) => {
                    ws.write_number(row, col, n)?;
                }
                CellSource::Text(s) if s.is_empty() => {}
                CellSource::Text(s) => {
                    ws.write_string(row, col, s.as_str())?;
                }
            }
        }
    }
    Ok(())
}

enum CellSource {
    Formula(String),
    Number(f64),
    Text(String),
}

fn extract_cell_source(cell: &Block) -> CellSource {
    // Detect a single @formula inline as the cell body.
    if let Body::Text(pieces) = &cell.body {
        let mut formula: Option<&Block> = None;
        let mut had_other = false;
        for p in pieces {
            match p {
                TextPiece::Literal { text, .. } => {
                    if !text.trim().is_empty() {
                        had_other = true;
                    }
                }
                TextPiece::Inline(b) if b.name == "formula" => {
                    if formula.is_some() {
                        had_other = true;
                    }
                    formula = Some(b);
                }
                TextPiece::Inline(_) => had_other = true,
            }
        }
        if let (Some(f), false) = (formula, had_other) {
            let text = f.plain_text().unwrap_or_default();
            // Excel doesn't accept a leading `=` inside the formula
            // string passed to write_formula (rust_xlsxwriter prepends
            // it). Strip any stray `=` defensively.
            let trimmed = text.trim_start_matches('=').trim().to_string();
            return CellSource::Formula(trimmed);
        }
    }
    let raw = cell.plain_text().unwrap_or_default();
    if let Ok(n) = raw.trim().parse::<f64>() {
        // Only treat as number if the parse covered the entire trimmed
        // body — otherwise something like "3 weeks" would silently
        // become 3.
        if raw.trim().chars().all(|c| {
            c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E'
        }) {
            return CellSource::Number(n);
        }
    }
    CellSource::Text(raw)
}

/// Parse a Stem cell address like "A1" or "AB12" into 0-based (col, row).
///
/// Re-implements what `stem-exports/html` does, since each backend's
/// dependency surface differs. If the input doesn't look like a single
/// cell address (e.g. "A1:B5"), returns None and we silently skip.
fn parse_address(s: &str) -> Option<(u16, u32)> {
    if s.contains(':') {
        return None;
    }
    let split = s.find(|c: char| c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let (col, row) = s.split_at(split);
    let mut n: u32 = 0;
    for c in col.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        n = n * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
    }
    if n == 0 {
        return None;
    }
    let col_idx: u16 = (n - 1).try_into().ok()?;
    let row_n: u32 = row.parse().ok()?;
    if row_n == 0 {
        return None;
    }
    Some((col_idx, row_n - 1))
}

/// Sanitize a sheet name to Excel's rules: max 31 chars, no characters
/// from \\ / ? * [ ]. Fall back to a numbered default if unrecoverable.
fn sanitize_sheet_name(name: &str, fallback_index: usize) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '\\' | '/' | '?' | '*' | '[' | ']'))
        .collect();
    let trimmed: String = cleaned.chars().take(31).collect();
    if trimmed.trim().is_empty() {
        format!("Sheet{}", fallback_index)
    } else {
        trimmed
    }
}

// `formula` is used by the import side too; reference it to keep the
// dep edge explicit even if this module doesn't call into it directly
// (yet — when we add format mapping we'll use formula::format_value).
#[allow(dead_code)]
fn _force_formula_link() -> &'static str {
    let _ = formula::FormulaError::UnexpectedEqualsPrefix.code();
    "ok"
}
