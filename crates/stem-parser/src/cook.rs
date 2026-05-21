//! Typed-tree pass (cook).
//!
//! Today this module handles **sheet-specific desugaring**: `fill` and
//! `source` blocks are expanded into per-cell blocks so downstream
//! consumers see one uniform cell model.
//!
//! Future work: merge per-address cells (CP4), apply column/row
//! cascades (CP5).

use stem_core::ast::*;
use stem_core::diagnostic::Diagnostic;

use crate::csv::{parse_csv, CsvOptions};

/// Pluggable file resolver for `source[file:"…"]` blocks. Cook calls
/// `load` with the property's path string and either receives the file
/// contents or an error string used as a diagnostic message.
///
/// In a WASM/no-fs environment, pass `None` for `file_loader` — every
/// `source` block then becomes a hint diagnostic and an empty cell list.
pub trait FileLoader {
    fn load(&self, path: &str) -> Result<String, String>;
}

/// Cook configuration. All fields are optional with sensible defaults.
#[derive(Default)]
pub struct CookOptions<'a> {
    pub file_loader: Option<&'a dyn FileLoader>,
}

/// Output of `cook_document_with`. Cook diagnostics are separate
/// from parser/validator diagnostics so callers can route them.
#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // `diagnostics` is part of the public API; read by external callers
pub struct CookResult {
    pub document: Document,
    pub diagnostics: Vec<Diagnostic>,
}

/// Convenience: cook with default options (no file loader). `source`
/// blocks become diagnostics + no-op.
pub fn cook_document(doc: &Document) -> Document {
    cook_document_with(doc, &CookOptions::default()).document
}

/// Cook with explicit options. Use this when you want to supply a
/// `FileLoader` (CLI case) and/or capture cook-time diagnostics.
pub fn cook_document_with(doc: &Document, opts: &CookOptions) -> CookResult {
    let mut cooker = Cooker {
        opts,
        diagnostics: Vec::new(),
    };
    let blocks = doc.blocks.iter().map(|b| cooker.cook_block(b)).collect();
    CookResult {
        document: Document {
            metadata: doc.metadata.clone(),
            blocks,
        },
        diagnostics: cooker.diagnostics,
    }
}

struct Cooker<'a> {
    opts: &'a CookOptions<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Cooker<'a> {
    fn cook_block(&mut self, block: &Block) -> Block {
        if block.name == "sheet" {
            return self.cook_sheet(block);
        }
        match &block.body {
            Body::Children(kids) => {
                let new_kids: Vec<Block> = kids.iter().map(|b| self.cook_block(b)).collect();
                Block {
                    body: Body::Children(new_kids),
                    ..block.clone()
                }
            }
            _ => block.clone(),
        }
    }

    fn cook_sheet(&mut self, sheet: &Block) -> Block {
        let kids = match &sheet.body {
            Body::Children(k) => k,
            _ => return sheet.clone(),
        };

        // Pass 1: desugar fill/source into per-cell blocks.
        let mut desugared: Vec<Block> = Vec::with_capacity(kids.len());
        for child in kids {
            match child.name.as_str() {
                "fill" => desugared.extend(desugar_fill(child)),
                "source" => desugared.extend(self.desugar_source(child)),
                _ => desugared.push(child.clone()),
            }
        }

        // Pass 2: merge cells with the same address (later overrides earlier).
        let merged = self.merge_cells(desugared);

        // Pass 3: cascade col/row/format properties down to matching cells.
        // Cells keep their own properties as the highest-specificity layer.
        let cascaded = self.cascade_cells(merged);

        Block {
            body: Body::Children(cascaded),
            ..sheet.clone()
        }
    }

    /// Walk a sheet's children and apply column/row/format properties
    /// to every cell they target. Cell-level properties always win.
    /// Within the cascade layer, later wins (CSS-style). The cascade
    /// "rule" blocks (`col`, `row`, `format`) are **removed** from the
    /// output — they're consumed by this pass. Cells and other blocks
    /// (`named`, `chart`) pass through.
    fn cascade_cells(&mut self, kids: Vec<Block>) -> Vec<Block> {
        // Accumulator: list of (scope, props) in source order. We walk
        // forward through `kids` and grow this list as we encounter
        // `col` / `row` / `format` blocks. When we hit a `cell`, we apply
        // every accumulated rule that targets the cell's address.
        let mut rules: Vec<(Scope, Vec<Property>)> = Vec::new();
        let mut output: Vec<Block> = Vec::with_capacity(kids.len());

        for kid in kids {
            match kid.name.as_str() {
                "col" => {
                    if let Some(scope) = kid.prop_str("at").and_then(parse_col_scope) {
                        rules.push((scope, cascadable_props(&kid)));
                    }
                    // don't keep the rule block in output
                }
                "row" => {
                    if let Some(scope) = kid.prop_str("at").and_then(parse_row_scope) {
                        rules.push((scope, cascadable_props(&kid)));
                    }
                }
                "format" => {
                    if let Some(scope) = kid.prop_str("at").and_then(parse_any_scope) {
                        rules.push((scope, cascadable_props(&kid)));
                    }
                }
                "cell" => {
                    let cooked = self.apply_cascade(&kid, &rules);
                    output.push(cooked);
                }
                _ => output.push(kid),
            }
        }

        output
    }

    fn apply_cascade(&mut self, cell: &Block, rules: &[(Scope, Vec<Property>)]) -> Block {
        let (col, row) = match cell.prop_str("at").and_then(parse_simple_address) {
            Some(addr) => addr,
            None => return cell.clone(),
        };

        // Build the cascaded property set in source order.
        let mut props: Vec<Property> = Vec::new();
        for (scope, rule_props) in rules {
            if scope.contains(col, row) {
                for p in rule_props {
                    upsert_property(&mut props, p);
                }
            }
        }
        // Then merge in cell's own properties (cell layer always wins).
        for p in &cell.properties {
            upsert_property(&mut props, p);
        }

        Block {
            properties: props,
            ..cell.clone()
        }
    }

    /// Walk a sheet's children, merging any `cell[at:X]` blocks with
    /// the same address. The first occurrence's position in the output
    /// is preserved; subsequent occurrences contribute their properties
    /// (later wins per key) and their body (replaces the earlier one if
    /// present, with a `type.duplicate_cell_body` warning when both
    /// supply a body).
    ///
    /// Non-cell blocks pass through in source order — `col[at:B]`,
    /// `format[at:...]`, `named[...]`, etc. stay where the author put
    /// them, so cascading still happens in declaration order.
    fn merge_cells(&mut self, kids: Vec<Block>) -> Vec<Block> {
        use std::collections::HashMap;

        let mut output: Vec<Block> = Vec::with_capacity(kids.len());
        let mut addr_to_idx: HashMap<String, usize> = HashMap::new();

        for kid in kids {
            if kid.name != "cell" {
                output.push(kid);
                continue;
            }
            let addr = kid.prop_str("at").map(|s| s.to_string());
            match addr {
                None => {
                    // Cell without `at:` — validator catches the missing prop.
                    output.push(kid);
                }
                Some(addr) => match addr_to_idx.get(&addr) {
                    Some(&idx) => {
                        // swap-remove style: take a clone, replace in place
                        let existing = output[idx].clone();
                        output[idx] = self.merge_two_cells(existing, kid);
                    }
                    None => {
                        let idx = output.len();
                        addr_to_idx.insert(addr, idx);
                        output.push(kid);
                    }
                },
            }
        }

        output
    }

    fn merge_two_cells(&mut self, earlier: Block, later: Block) -> Block {
        // Merge properties: later wins per key
        let mut props: Vec<Property> = earlier.properties.clone();
        for new_prop in &later.properties {
            if let Some(existing) = props.iter_mut().find(|p| p.key == new_prop.key) {
                *existing = new_prop.clone();
            } else {
                props.push(new_prop.clone());
            }
        }

        // Body merge rules per spec §10 "Cell merge semantics"
        let body = match (&earlier.body, &later.body) {
            (_, Body::None) => earlier.body.clone(),
            (Body::None, _) => later.body.clone(),
            _ => {
                self.diagnostics.push(Diagnostic::warning(
                    "type.duplicate_cell_body",
                    "two `cell[at:…]` blocks at the same address both supply a body — \
                     using the later",
                    later.span,
                ));
                later.body.clone()
            }
        };

        Block {
            name: "cell".to_string(),
            name_span: earlier.name_span,
            properties: props,
            body,
            inline_form: false,
            span: earlier.span,
        }
    }

    fn desugar_source(&mut self, source: &Block) -> Vec<Block> {
        let file = match source.prop_str("file") {
            Some(f) => f,
            None => {
                self.diagnostics.push(Diagnostic::error(
                    "cook.source_missing_file",
                    "`source` requires a `file:` property",
                    source.span,
                ));
                return Vec::new();
            }
        };

        let loader = match self.opts.file_loader {
            Some(l) => l,
            None => {
                self.diagnostics.push(Diagnostic::hint(
                    "cook.no_file_loader",
                    format!(
                        "`source[file:\"{}\"]` skipped — no file loader configured in this environment",
                        file
                    ),
                    source.span,
                ));
                return Vec::new();
            }
        };

        let body = match loader.load(file) {
            Ok(s) => s,
            Err(e) => {
                self.diagnostics.push(Diagnostic::error(
                    "cook.source_load_failed",
                    format!("failed to load `{}`: {}", file, e),
                    source.span,
                ));
                return Vec::new();
            }
        };

        let at = source.prop_str("at").unwrap_or("A1");
        let sep_str = source.prop_str("sep").unwrap_or(",");
        let sep = sep_str.chars().next().unwrap_or(',');
        let has_header = source
            .prop("has-header")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let table = parse_csv(
            &body,
            CsvOptions {
                separator: sep,
                has_header,
            },
        );
        let (start_col, start_row) = parse_simple_address(at).unwrap_or((0, 0));

        let mut out = Vec::new();
        for (r, row) in table.rows.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                if value.is_empty() {
                    continue;
                }
                let addr = format_address(start_col + c as u32, start_row + r as u32);
                out.push(synth_cell(addr, value, source));
            }
        }
        out
    }
}

/// Expand a `fill[at:X, sep:?, has-header:?]("csv body")` block into
/// per-cell blocks.
fn desugar_fill(fill: &Block) -> Vec<Block> {
    let at = fill.prop_str("at").unwrap_or("A1");
    let sep_str = fill.prop_str("sep").unwrap_or(",");
    let sep = sep_str.chars().next().unwrap_or(',');
    let has_header = fill
        .prop("has-header")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let csv_text = match &fill.body {
        Body::Text(pieces) => collect_text(pieces),
        _ => return Vec::new(),
    };

    let table = parse_csv(
        &csv_text,
        CsvOptions {
            separator: sep,
            has_header,
        },
    );

    let (start_col, start_row) = parse_simple_address(at).unwrap_or((0, 0));

    let mut out = Vec::with_capacity(table.rows.iter().map(|r| r.len()).sum());
    for (r, row) in table.rows.iter().enumerate() {
        for (c, value) in row.iter().enumerate() {
            if value.is_empty() {
                continue; // skip empty cells — they're not addressable values
            }
            let addr = format_address(start_col + c as u32, start_row + r as u32);
            out.push(synth_cell(addr, value, fill));
        }
    }
    out
}

/// Build a `cell[at:ADDR](value)` block. Spans point at the originating
/// `fill` block so diagnostics still locate the source.
fn synth_cell(addr: String, value: &str, origin: &Block) -> Block {
    Block {
        name: "cell".to_string(),
        name_span: origin.name_span,
        properties: vec![Property {
            key: "at".to_string(),
            key_span: origin.span,
            value: PropertyValue::Bare(addr),
            value_span: origin.span,
        }],
        body: Body::Text(vec![TextPiece::Literal {
            text: value.to_string(),
            span: origin.span,
        }]),
        inline_form: false,
        span: origin.span,
    }
}

fn collect_text(pieces: &[TextPiece]) -> String {
    let mut s = String::new();
    for p in pieces {
        if let TextPiece::Literal { text, .. } = p {
            s.push_str(text);
        }
    }
    s
}

// -----------------------------------------------------------
// Cascade-scope and property helpers
// -----------------------------------------------------------

/// Set of cell positions a cascade rule applies to.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Scope {
    /// Whole column (col_index, all rows).
    Column(u32),
    /// Whole row (all cols, row_index).
    Row(u32),
    /// Rectangle (top-left inclusive, bottom-right inclusive).
    Rect((u32, u32), (u32, u32)),
    /// A single cell (degenerate rectangle).
    Cell(u32, u32),
}

impl Scope {
    fn contains(&self, col: u32, row: u32) -> bool {
        match *self {
            Scope::Column(c) => c == col,
            Scope::Row(r) => r == row,
            Scope::Rect((c1, r1), (c2, r2)) => col >= c1 && col <= c2 && row >= r1 && row <= r2,
            Scope::Cell(c, r) => c == col && r == row,
        }
    }
}

/// Parse a column scope from a `col[at:X]` property — just a column letter.
fn parse_col_scope(s: &str) -> Option<Scope> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Reject if any digit appears — that's a cell address, not a column.
    if trimmed.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    parse_column(trimmed).map(Scope::Column)
}

/// Parse a row scope from a `row[at:N]` property — just a row number.
fn parse_row_scope(s: &str) -> Option<Scope> {
    let n: u32 = s.trim().parse().ok()?;
    if n == 0 {
        return None;
    }
    Some(Scope::Row(n - 1))
}

/// Parse any address shape — cell, column, row, or range — for `format[at:…]`.
fn parse_any_scope(s: &str) -> Option<Scope> {
    let trimmed = s.trim();
    if let Some(rect) = parse_range(trimmed) {
        return Some(rect);
    }
    if let Some(addr) = parse_simple_address(trimmed) {
        return Some(Scope::Cell(addr.0, addr.1));
    }
    if let Some(col) = parse_col_scope(trimmed) {
        return Some(col);
    }
    if let Some(row) = parse_row_scope(trimmed) {
        return Some(row);
    }
    None
}

/// Parse an Excel-style range like "B2:B4" or "A1:C5" into a Scope::Rect.
fn parse_range(s: &str) -> Option<Scope> {
    let (lhs, rhs) = s.split_once(':')?;
    let (c1, r1) = parse_simple_address(lhs.trim())?;
    let (c2, r2) = parse_simple_address(rhs.trim())?;
    let (lo_c, hi_c) = if c1 <= c2 { (c1, c2) } else { (c2, c1) };
    let (lo_r, hi_r) = if r1 <= r2 { (r1, r2) } else { (r2, r1) };
    Some(Scope::Rect((lo_c, lo_r), (hi_c, hi_r)))
}

/// Extract the subset of a rule block's properties that cascade.
/// Cascading set (Stem 1.0): `bg`, `fmt`, `weight`, `align`, `valign`,
/// `style`, `decoration`. Anything else (`at`, `width`, `height`,
/// `freeze`, `kind`, …) stays on its declaring block.
fn cascadable_props(b: &Block) -> Vec<Property> {
    const CASCADE: &[&str] = &[
        "bg", "fmt", "weight", "align", "valign", "style", "decoration", "color",
    ];
    b.properties
        .iter()
        .filter(|p| CASCADE.contains(&p.key.as_str()))
        .cloned()
        .collect()
}

fn upsert_property(props: &mut Vec<Property>, new_prop: &Property) {
    if let Some(existing) = props.iter_mut().find(|p| p.key == new_prop.key) {
        *existing = new_prop.clone();
    } else {
        props.push(new_prop.clone());
    }
}

// -----------------------------------------------------------
// Address utilities (single-cell only; ranges deferred to a later CP)
// -----------------------------------------------------------

/// Parse a single-cell address like "A1" or "AB12" into (col_index, row_index).
/// Returns None if the input doesn't look like a single cell.
fn parse_simple_address(s: &str) -> Option<(u32, u32)> {
    if s.is_empty() {
        return None;
    }
    let split = s.find(|c: char| c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let (col, row) = s.split_at(split);
    let col_idx = parse_column(col)?;
    let row_n: u32 = row.parse().ok()?;
    if row_n == 0 {
        return None;
    }
    Some((col_idx, row_n - 1))
}

/// Parse a column letter ("A"=0, "Z"=25, "AA"=26, ...) into a 0-based index.
fn parse_column(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut n: u32 = 0;
    for c in s.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        let v = (c.to_ascii_uppercase() as u32) - (b'A' as u32) + 1;
        n = n.checked_mul(26)?.checked_add(v)?;
    }
    Some(n - 1)
}

/// Format a (col_index, row_index) as an "A1"-style address.
fn format_address(col: u32, row: u32) -> String {
    format!("{}{}", format_column(col), row + 1)
}

fn format_column(mut n: u32) -> String {
    // 0 → "A", 25 → "Z", 26 → "AA", 27 → "AB", ...
    let mut s = String::new();
    n += 1;
    while n > 0 {
        let r = (n - 1) % 26;
        s.insert(0, (b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_round_trip() {
        for i in [0, 1, 25, 26, 27, 51, 52, 701, 702, 703] {
            let s = format_column(i);
            assert_eq!(parse_column(&s), Some(i), "round trip failed for {}", i);
        }
        assert_eq!(format_column(0), "A");
        assert_eq!(format_column(25), "Z");
        assert_eq!(format_column(26), "AA");
        assert_eq!(format_column(27), "AB");
        assert_eq!(format_column(51), "AZ");
        assert_eq!(format_column(52), "BA");
    }

    #[test]
    fn parse_single_addresses() {
        assert_eq!(parse_simple_address("A1"), Some((0, 0)));
        assert_eq!(parse_simple_address("B5"), Some((1, 4)));
        assert_eq!(parse_simple_address("Z100"), Some((25, 99)));
        assert_eq!(parse_simple_address("AA1"), Some((26, 0)));
        assert_eq!(parse_simple_address("AB12"), Some((27, 11)));
    }

    #[test]
    fn parse_rejects_bad_addresses() {
        assert_eq!(parse_simple_address(""), None);
        assert_eq!(parse_simple_address("123"), None); // no column
        assert_eq!(parse_simple_address("ABC"), None); // no row
        assert_eq!(parse_simple_address("A0"), None); // row must be >= 1
    }

    #[test]
    fn format_then_parse() {
        for col in [0, 1, 5, 26, 700] {
            for row in [0, 1, 4, 99] {
                let s = format_address(col, row);
                assert_eq!(parse_simple_address(&s), Some((col, row)), "addr {}", s);
            }
        }
    }

    // ---- Desugar tests ----

    use crate::parse;

    fn parse_and_cook(src: &str) -> Document {
        let r = parse(src);
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        cook_document(&r.document)
    }

    fn sheet_cells(doc: &Document) -> Vec<(String, String)> {
        let sheet = &doc.blocks[0];
        let kids = match &sheet.body {
            Body::Children(k) => k,
            _ => panic!("no children"),
        };
        kids.iter()
            .filter(|b| b.name == "cell")
            .map(|c| {
                let at = c.prop_str("at").unwrap_or("?").to_string();
                let body = c.plain_text().unwrap_or_default();
                (at, body)
            })
            .collect()
    }

    #[test]
    fn fill_at_a1_basic_grid() {
        let src = r#"[type:sheet]
sheet{
  fill[at:A1]("
    Item, Revenue, Margin
    Widget, 42000, 0.35
    Total, 80500, 0.40
  ")
}"#;
        let doc = parse_and_cook(src);
        let cells = sheet_cells(&doc);
        assert_eq!(
            cells,
            vec![
                ("A1".into(), "Item".into()),
                ("B1".into(), "Revenue".into()),
                ("C1".into(), "Margin".into()),
                ("A2".into(), "Widget".into()),
                ("B2".into(), "42000".into()),
                ("C2".into(), "0.35".into()),
                ("A3".into(), "Total".into()),
                ("B3".into(), "80500".into()),
                ("C3".into(), "0.40".into()),
            ]
        );
    }

    #[test]
    fn fill_anchored_at_b3() {
        let src = r#"[type:sheet]
sheet{
  fill[at:B3]("
    x, y
    1, 2
  ")
}"#;
        let doc = parse_and_cook(src);
        let cells = sheet_cells(&doc);
        assert_eq!(
            cells,
            vec![
                ("B3".into(), "x".into()),
                ("C3".into(), "y".into()),
                ("B4".into(), "1".into()),
                ("C4".into(), "2".into()),
            ]
        );
    }

    #[test]
    fn fill_with_quoted_formula() {
        let src = r#"[type:sheet]
sheet{
  fill[at:A1]("
    Total, =SUM(B2:B4)
  ")
}"#;
        let doc = parse_and_cook(src);
        let cells = sheet_cells(&doc);
        assert_eq!(
            cells,
            vec![
                ("A1".into(), "Total".into()),
                ("B1".into(), "=SUM(B2:B4)".into()),
            ]
        );
    }

    #[test]
    fn fill_with_tsv_separator() {
        let src = "[type:sheet]\nsheet{\n  fill[at:A1, sep:\"\\t\"](\"x\\ty\\n1\\t2\")\n}\n";
        // Note: parsing of `\t` inside property value is via the quoted-string escape;
        // when we get the property value, sep_str should be "\t" (one char).
        let doc = parse_and_cook(src);
        let cells = sheet_cells(&doc);
        assert_eq!(
            cells,
            vec![
                ("A1".into(), "x".into()),
                ("B1".into(), "y".into()),
                ("A2".into(), "1".into()),
                ("B2".into(), "2".into()),
            ]
        );
    }

    // ---- source desugar tests ----

    use crate::cook_document_with;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MapLoader {
        files: Mutex<HashMap<String, String>>,
    }
    impl MapLoader {
        fn new() -> Self {
            Self { files: Mutex::new(HashMap::new()) }
        }
        fn add(&self, path: &str, content: &str) {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_string(), content.to_string());
        }
    }
    impl FileLoader for MapLoader {
        fn load(&self, path: &str) -> Result<String, String> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| format!("no such file: {}", path))
        }
    }

    #[test]
    fn source_with_loader_desugars_to_cells() {
        let loader = MapLoader::new();
        loader.add("q4.csv", "Item,Revenue\nWidget,42000\n");
        let r = parse(
            "[type:sheet]\nsheet{ source[file:\"q4.csv\", at:A1] }",
        );
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        let result = cook_document_with(
            &r.document,
            &CookOptions {
                file_loader: Some(&loader),
            },
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let cells = sheet_cells(&result.document);
        assert_eq!(
            cells,
            vec![
                ("A1".into(), "Item".into()),
                ("B1".into(), "Revenue".into()),
                ("A2".into(), "Widget".into()),
                ("B2".into(), "42000".into()),
            ]
        );
    }

    #[test]
    fn source_without_loader_emits_hint() {
        let r = parse(
            "[type:sheet]\nsheet{ source[file:\"q4.csv\", at:A1] }",
        );
        let result = cook_document_with(&r.document, &CookOptions::default());
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "cook.no_file_loader"));
        // sheet has no children left (the source was skipped)
        let sheet = &result.document.blocks[0];
        if let Body::Children(kids) = &sheet.body {
            assert!(kids.is_empty());
        }
    }

    #[test]
    fn source_missing_file_property_errors() {
        let r = parse("[type:sheet]\nsheet{ source[at:A1] }");
        let result = cook_document_with(&r.document, &CookOptions::default());
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "cook.source_missing_file"));
    }

    #[test]
    fn source_load_failure_errors() {
        let loader = MapLoader::new(); // no files registered
        let r = parse(
            "[type:sheet]\nsheet{ source[file:\"missing.csv\", at:A1] }",
        );
        let result = cook_document_with(
            &r.document,
            &CookOptions {
                file_loader: Some(&loader),
            },
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "cook.source_load_failed"));
    }

    #[test]
    fn source_anchored_at_non_origin() {
        let loader = MapLoader::new();
        loader.add("d.csv", "a,b\n1,2\n");
        let r = parse("[type:sheet]\nsheet{ source[file:\"d.csv\", at:C5] }");
        let result = cook_document_with(
            &r.document,
            &CookOptions {
                file_loader: Some(&loader),
            },
        );
        assert!(result.diagnostics.is_empty());
        let cells = sheet_cells(&result.document);
        assert_eq!(
            cells,
            vec![
                ("C5".into(), "a".into()),
                ("D5".into(), "b".into()),
                ("C6".into(), "1".into()),
                ("D6".into(), "2".into()),
            ]
        );
    }

    // ---- cell merge tests ----

    #[test]
    fn override_after_fill_merges_properties_keeps_value() {
        let src = r#"[type:sheet]
sheet{
  fill[at:A1]("a, b\nc, d")
  cell[at:B2, bg:yellow]
}"#;
        let doc = parse_and_cook(src);
        let cells = sheet_cells(&doc);
        // B2 should still have value "d" but with bg property
        assert_eq!(cells.len(), 4);
        let sheet = &doc.blocks[0];
        let kids = match &sheet.body {
            Body::Children(k) => k,
            _ => panic!(),
        };
        let b2 = kids
            .iter()
            .find(|c| c.name == "cell" && c.prop_str("at") == Some("B2"))
            .expect("B2 missing");
        assert_eq!(b2.plain_text().as_deref(), Some("d"));
        assert_eq!(b2.prop_str("bg"), Some("yellow"));
    }

    #[test]
    fn later_property_wins_per_key() {
        let src = r#"[type:sheet]
sheet{
  cell[at:A1, bg:gray, weight:regular](v)
  cell[at:A1, bg:yellow]
}"#;
        let doc = parse_and_cook(src);
        let cells = sheet_cells(&doc);
        assert_eq!(cells.len(), 1);
        let sheet = &doc.blocks[0];
        let kids = match &sheet.body {
            Body::Children(k) => k,
            _ => panic!(),
        };
        let a1 = kids.iter().find(|c| c.name == "cell").unwrap();
        assert_eq!(a1.plain_text().as_deref(), Some("v"));
        assert_eq!(a1.prop_str("bg"), Some("yellow"), "later bg should win");
        assert_eq!(a1.prop_str("weight"), Some("regular"), "earlier prop preserved");
    }

    #[test]
    fn duplicate_body_emits_warning() {
        use stem_core::diagnostic::Severity;
        let src = r#"[type:sheet]
sheet{
  cell[at:A1](first)
  cell[at:A1](second)
}"#;
        let r = parse(src);
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        let result = cook_document_with(&r.document, &CookOptions::default());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "type.duplicate_cell_body" && d.severity == Severity::Warning),
            "diags: {:?}",
            result.diagnostics
        );
        // and the later body wins
        let kids = match &result.document.blocks[0].body {
            Body::Children(k) => k,
            _ => panic!(),
        };
        let a1 = kids.iter().find(|c| c.name == "cell").unwrap();
        assert_eq!(a1.plain_text().as_deref(), Some("second"));
    }

    #[test]
    fn merge_preserves_first_position() {
        let src = r#"[type:sheet]
sheet{
  cell[at:A1](first)
  cell[at:B1](second)
  cell[at:A1, bg:red]
}"#;
        let doc = parse_and_cook(src);
        let sheet = &doc.blocks[0];
        let kids = match &sheet.body {
            Body::Children(k) => k,
            _ => panic!(),
        };
        let cell_addrs: Vec<&str> = kids
            .iter()
            .filter(|c| c.name == "cell")
            .map(|c| c.prop_str("at").unwrap())
            .collect();
        // A1 should keep its first position, B1 second
        assert_eq!(cell_addrs, vec!["A1", "B1"]);
    }

    #[test]
    fn merge_preserves_non_cascade_blocks() {
        // `col` is a cascade rule — consumed by CP5. `named` is not a
        // cascade rule — it survives into the cooked output.
        let src = r#"[type:sheet]
sheet{
  named[name:Revenue, at:"B2:B4"]
  cell[at:A1](one)
  cell[at:A1, bg:yellow]
}"#;
        let doc = parse_and_cook(src);
        let kids = match &doc.blocks[0].body {
            Body::Children(k) => k,
            _ => panic!(),
        };
        assert_eq!(kids[0].name, "named");
        assert_eq!(kids[1].name, "cell");
        assert_eq!(kids[1].prop_str("at"), Some("A1"));
        assert_eq!(kids[1].prop_str("bg"), Some("yellow"));
        assert_eq!(kids.len(), 2);
    }

    // ---- cascade tests ----

    fn cell_at<'a>(doc: &'a Document, addr: &str) -> &'a Block {
        let kids = match &doc.blocks[0].body {
            Body::Children(k) => k,
            _ => panic!(),
        };
        kids.iter()
            .find(|b| b.name == "cell" && b.prop_str("at") == Some(addr))
            .unwrap_or_else(|| panic!("no cell at {}", addr))
    }

    #[test]
    fn column_property_cascades_to_cell() {
        let src = r#"[type:sheet]
sheet{
  col[at:B, fmt:currency]
  cell[at:A1](label)
  cell[at:B1](42000)
  cell[at:C1](other)
}"#;
        let doc = parse_and_cook(src);
        assert_eq!(cell_at(&doc, "A1").prop_str("fmt"), None);
        assert_eq!(cell_at(&doc, "B1").prop_str("fmt"), Some("currency"));
        assert_eq!(cell_at(&doc, "C1").prop_str("fmt"), None);
    }

    #[test]
    fn row_property_cascades_to_cell() {
        let src = r#"[type:sheet]
sheet{
  row[at:1, weight:bold]
  cell[at:A1](label)
  cell[at:A2](body)
}"#;
        let doc = parse_and_cook(src);
        assert_eq!(cell_at(&doc, "A1").prop_str("weight"), Some("bold"));
        assert_eq!(cell_at(&doc, "A2").prop_str("weight"), None);
    }

    #[test]
    fn cell_property_wins_over_cascade() {
        let src = r#"[type:sheet]
sheet{
  col[at:B, fmt:currency]
  cell[at:B1, fmt:percent](v)
}"#;
        let doc = parse_and_cook(src);
        assert_eq!(cell_at(&doc, "B1").prop_str("fmt"), Some("percent"));
    }

    #[test]
    fn cascade_layers_combine_per_property() {
        // col supplies fmt, row supplies weight — cell ends up with both
        let src = r#"[type:sheet]
sheet{
  col[at:B, fmt:currency]
  row[at:1, weight:bold]
  cell[at:B1](v)
}"#;
        let doc = parse_and_cook(src);
        let b1 = cell_at(&doc, "B1");
        assert_eq!(b1.prop_str("fmt"), Some("currency"));
        assert_eq!(b1.prop_str("weight"), Some("bold"));
    }

    #[test]
    fn later_cascade_rule_wins() {
        let src = r#"[type:sheet]
sheet{
  col[at:B, fmt:currency]
  col[at:B, fmt:percent]
  cell[at:B1](v)
}"#;
        let doc = parse_and_cook(src);
        assert_eq!(cell_at(&doc, "B1").prop_str("fmt"), Some("percent"));
    }

    #[test]
    fn format_with_range_cascades() {
        let src = r#"[type:sheet]
sheet{
  format[at:"A1:B2", bg:gray]
  cell[at:A1](a)
  cell[at:B2](b)
  cell[at:C3](c)
}"#;
        let doc = parse_and_cook(src);
        assert_eq!(cell_at(&doc, "A1").prop_str("bg"), Some("gray"));
        assert_eq!(cell_at(&doc, "B2").prop_str("bg"), Some("gray"));
        assert_eq!(cell_at(&doc, "C3").prop_str("bg"), None);
    }

    #[test]
    fn cascade_consumes_col_row_format_blocks() {
        // After cascade, no col/row/format remain in the output
        let src = r#"[type:sheet]
sheet{
  col[at:B, fmt:currency]
  row[at:1, bg:gray]
  format[at:"A1:C5", weight:bold]
  cell[at:A1](v)
}"#;
        let doc = parse_and_cook(src);
        let kids = match &doc.blocks[0].body {
            Body::Children(k) => k,
            _ => panic!(),
        };
        for k in kids {
            assert!(
                k.name == "cell" || k.name == "named" || k.name == "chart",
                "unexpected leftover: {}",
                k.name
            );
        }
    }

    #[test]
    fn control_properties_do_not_cascade() {
        // `width` is a column control property; it must not appear on cells
        let src = r#"[type:sheet]
sheet{
  col[at:B, width:120]
  cell[at:B1](v)
}"#;
        let doc = parse_and_cook(src);
        assert_eq!(cell_at(&doc, "B1").prop_str("width"), None);
    }

    #[test]
    fn fill_preserves_sibling_cells() {
        let src = r#"[type:sheet]
sheet{
  fill[at:A1]("
    a, b
  ")
  cell[at:C5](standalone)
}"#;
        let doc = parse_and_cook(src);
        let cells = sheet_cells(&doc);
        assert_eq!(
            cells,
            vec![
                ("A1".into(), "a".into()),
                ("B1".into(), "b".into()),
                ("C5".into(), "standalone".into()),
            ]
        );
    }
}
