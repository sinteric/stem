//! RFC 4180-style CSV parser.
//!
//! Used by sheet `fill[at:X](csv body)` and `source[file:...]`. Self-
//! contained, no Stem dependencies — could move to its own crate if we
//! ever publish it.
//!
//! Rules:
//! - Rows separated by `\n` (treat `\r\n` as `\n`).
//! - Cells separated by `sep` (default `,`).
//! - A cell may be wrapped in `"…"` to allow literal separators or
//!   newlines inside it. `""` doubled inside a quoted cell becomes a
//!   literal `"`.
//! - Leading/trailing whitespace around an unquoted cell is trimmed.
//! - Trailing newline at EOF is ignored (does not produce an empty row).
//! - Empty rows are skipped (rows where every cell is empty after trim).

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsvOptions {
    /// Field separator. Single character.
    pub separator: char,
    /// Treat the first row as a header (caller decides what to do with it).
    pub has_header: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            separator: ',',
            has_header: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsvTable {
    pub rows: Vec<Vec<String>>,
    pub has_header: bool,
}

/// Parse a CSV string into a table of cells.
pub fn parse_csv(input: &str, opts: CsvOptions) -> CsvTable {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut cur_row: Vec<String> = Vec::new();
    let mut cur_cell = String::new();
    let mut in_quotes = false;
    // "we haven't yet seen a non-whitespace char in this cell" —
    // stays true while consuming leading whitespace.
    let mut at_cell_start = true;
    // "this cell was opened with a quote" — when true, don't trim.
    let mut cell_was_quoted = false;

    let chars: Vec<char> = input.chars().collect();
    let sep = opts.separator;

    let push_cell = |cur_cell: &mut String,
                     cell_was_quoted: &mut bool,
                     cur_row: &mut Vec<String>| {
        let s = if *cell_was_quoted {
            std::mem::take(cur_cell)
        } else {
            std::mem::take(cur_cell).trim().to_string()
        };
        cur_row.push(s);
        *cell_was_quoted = false;
    };

    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];

        if in_quotes {
            if ch == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    cur_cell.push('"');
                    i += 2;
                    continue;
                }
                in_quotes = false;
                i += 1;
                continue;
            }
            cur_cell.push(ch);
            i += 1;
            continue;
        }

        match ch {
            '"' if at_cell_start => {
                in_quotes = true;
                cell_was_quoted = true;
                at_cell_start = false;
                // Discard any leading whitespace already accumulated.
                cur_cell.clear();
                i += 1;
            }
            c if c == sep => {
                push_cell(&mut cur_cell, &mut cell_was_quoted, &mut cur_row);
                at_cell_start = true;
                i += 1;
            }
            '\r' => {
                i += 1;
            }
            '\n' => {
                push_cell(&mut cur_cell, &mut cell_was_quoted, &mut cur_row);
                if !is_all_empty(&cur_row) {
                    rows.push(std::mem::take(&mut cur_row));
                } else {
                    cur_row.clear();
                }
                at_cell_start = true;
                i += 1;
            }
            other => {
                cur_cell.push(other);
                if !other.is_whitespace() {
                    at_cell_start = false;
                }
                i += 1;
            }
        }
    }

    if !cur_cell.is_empty() || !cur_row.is_empty() {
        push_cell(&mut cur_cell, &mut cell_was_quoted, &mut cur_row);
        if !is_all_empty(&cur_row) {
            rows.push(cur_row);
        }
    }

    CsvTable {
        rows,
        has_header: opts.has_header,
    }
}

fn is_all_empty(row: &[String]) -> bool {
    row.iter().all(|c| c.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Vec<Vec<String>> {
        parse_csv(s, CsvOptions::default()).rows
    }

    #[test]
    fn empty_input_produces_empty_table() {
        assert_eq!(parse(""), Vec::<Vec<String>>::new());
    }

    #[test]
    fn single_row_three_cells() {
        assert_eq!(
            parse("a,b,c"),
            vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]
        );
    }

    #[test]
    fn multiple_rows() {
        assert_eq!(
            parse("a,b\nc,d"),
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
            ]
        );
    }

    #[test]
    fn cells_trimmed_when_unquoted() {
        assert_eq!(
            parse("  a  ,  b  "),
            vec![vec!["a".to_string(), "b".to_string()]]
        );
    }

    #[test]
    fn quoted_cell_preserves_whitespace_and_separators() {
        assert_eq!(
            parse(r#""a,b","  c  ",d"#),
            vec![vec!["a,b".to_string(), "  c  ".to_string(), "d".to_string()]]
        );
    }

    #[test]
    fn doubled_quote_inside_quoted_cell() {
        assert_eq!(
            parse(r#""she said ""hi""","next""#),
            vec![vec![r#"she said "hi""#.to_string(), "next".to_string()]]
        );
    }

    #[test]
    fn quoted_cell_with_newline_inside() {
        assert_eq!(
            parse("\"line1\nline2\",b"),
            vec![vec!["line1\nline2".to_string(), "b".to_string()]]
        );
    }

    #[test]
    fn tab_separator() {
        let r = parse_csv("a\tb\tc", CsvOptions { separator: '\t', has_header: false });
        assert_eq!(r.rows, vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]);
    }

    #[test]
    fn semicolon_separator() {
        let r = parse_csv("a;b;c", CsvOptions { separator: ';', has_header: false });
        assert_eq!(r.rows, vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]);
    }

    #[test]
    fn formula_with_parens_and_commas_in_quotes() {
        // The kind of body that comes out of a stem fill block
        let r = parse(r#"Total,"=SUM(B2:B4)","=AVG(C2:C4)""#);
        assert_eq!(
            r,
            vec![vec![
                "Total".to_string(),
                "=SUM(B2:B4)".to_string(),
                "=AVG(C2:C4)".to_string(),
            ]]
        );
    }

    #[test]
    fn empty_rows_are_skipped() {
        assert_eq!(
            parse("a,b\n\n\nc,d\n"),
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
            ]
        );
    }

    #[test]
    fn crlf_line_endings() {
        assert_eq!(
            parse("a,b\r\nc,d\r\n"),
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
            ]
        );
    }

    #[test]
    fn empty_cells() {
        assert_eq!(
            parse("a,,c"),
            vec![vec!["a".to_string(), "".to_string(), "c".to_string()]]
        );
    }

    #[test]
    fn unicode_in_cells() {
        assert_eq!(
            parse("이름,값\n위젯,42"),
            vec![
                vec!["이름".to_string(), "값".to_string()],
                vec!["위젯".to_string(), "42".to_string()],
            ]
        );
    }

    #[test]
    fn realistic_fill_body() {
        let body = "
  Item,     Revenue,        Margin
  Widget,   42000,          0.35
  Total,    \"=SUM(B2:B4)\", \"=AVG(C2:C4)\"
";
        let rows = parse(body);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0][0], "Item");
        assert_eq!(rows[1][1], "42000");
        assert_eq!(rows[2][1], "=SUM(B2:B4)");
    }
}
