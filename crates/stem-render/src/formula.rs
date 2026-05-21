//! Spreadsheet formula engine. Lex → parse → evaluate.
//!
//! Supports the common case for v2.0 demos:
//! - Number literals (`42`, `-3.14`, `0.35`)
//! - Cell references (`A1`, `AB12`)
//! - Ranges inside function args (`B2:B6`)
//! - Operators: `+ - * / ^` and unary `-`
//! - Parens for grouping
//! - Functions: `SUM`, `AVERAGE`/`AVG`, `MIN`, `MAX`, `COUNT`,
//!   `IF(cond, then, else)`, `ABS`, `ROUND`
//!
//! NOT supported (yet): string concatenation, comparison operators,
//! boolean literals, nested ranges, structured references, lookups
//! (VLOOKUP/INDEX/MATCH). All easy to add later.

use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    Error(String),
}

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Str(s) => s.parse().ok(),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Error(_) => None,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Value::Error(_))
    }
}

/// AST for a parsed formula expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    CellRef(u32, u32),                   // (col, row), 0-based
    Range((u32, u32), (u32, u32)),       // ((c1, r1), (c2, r2))
    UnaryMinus(Box<Expr>),
    BinOp(Op, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

// ============================================================
// Public entry points
// ============================================================

/// Parse a formula source string. The leading `=` (if present) is stripped.
pub fn parse_formula(src: &str) -> Result<Expr, String> {
    let s = src.trim_start_matches('=').trim();
    let tokens = lex(s)?;
    let mut p = Parser { tokens, pos: 0 };
    let expr = p.parse_expr()?;
    if p.pos != p.tokens.len() {
        return Err(format!(
            "unexpected token after expression: {:?}",
            p.tokens[p.pos]
        ));
    }
    Ok(expr)
}

/// Evaluator context: looks up cell values by (col, row) address.
pub trait CellEnv {
    fn get(&self, col: u32, row: u32) -> Value;
}

/// Evaluate an expression in a given cell context.
pub fn eval(expr: &Expr, env: &dyn CellEnv) -> Value {
    match expr {
        Expr::Num(n) => Value::Num(*n),
        Expr::CellRef(c, r) => env.get(*c, *r),
        Expr::Range(_, _) => Value::Error("range used outside a function".into()),
        Expr::UnaryMinus(e) => match eval(e, env) {
            Value::Num(n) => Value::Num(-n),
            v if v.is_error() => v,
            other => Value::Error(format!("unary minus on non-number: {:?}", other)),
        },
        Expr::BinOp(op, a, b) => eval_binop(*op, a, b, env),
        Expr::Call(name, args) => eval_call(name, args, env),
    }
}

fn eval_binop(op: Op, a: &Expr, b: &Expr, env: &dyn CellEnv) -> Value {
    let av = eval(a, env);
    if av.is_error() {
        return av;
    }
    let bv = eval(b, env);
    if bv.is_error() {
        return bv;
    }
    let an = match av.as_number() {
        Some(n) => n,
        None => return Value::Error(format!("non-numeric LHS: {:?}", av)),
    };
    let bn = match bv.as_number() {
        Some(n) => n,
        None => return Value::Error(format!("non-numeric RHS: {:?}", bv)),
    };
    Value::Num(match op {
        Op::Add => an + bn,
        Op::Sub => an - bn,
        Op::Mul => an * bn,
        Op::Div => {
            if bn == 0.0 {
                return Value::Error("division by zero".into());
            }
            an / bn
        }
        Op::Pow => an.powf(bn),
    })
}

fn eval_call(name: &str, args: &[Expr], env: &dyn CellEnv) -> Value {
    // Materialize args into Vec<Value>; expand ranges to many values.
    let mut values: Vec<Value> = Vec::new();
    for arg in args {
        match arg {
            Expr::Range((c1, r1), (c2, r2)) => {
                for r in *r1..=*r2 {
                    for c in *c1..=*c2 {
                        values.push(env.get(c, r));
                    }
                }
            }
            other => values.push(eval(other, env)),
        }
    }

    let nums = || -> Vec<f64> {
        values
            .iter()
            .filter_map(|v| v.as_number())
            .collect()
    };

    let upper = name.to_uppercase();
    match upper.as_str() {
        "SUM" => Value::Num(nums().iter().sum()),
        "AVG" | "AVERAGE" => {
            let ns = nums();
            if ns.is_empty() {
                Value::Error("AVG of empty range".into())
            } else {
                Value::Num(ns.iter().sum::<f64>() / ns.len() as f64)
            }
        }
        "MIN" => {
            let ns = nums();
            ns.iter()
                .copied()
                .fold(None::<f64>, |acc, x| Some(acc.map_or(x, |a| a.min(x))))
                .map(Value::Num)
                .unwrap_or_else(|| Value::Error("MIN of empty range".into()))
        }
        "MAX" => {
            let ns = nums();
            ns.iter()
                .copied()
                .fold(None::<f64>, |acc, x| Some(acc.map_or(x, |a| a.max(x))))
                .map(Value::Num)
                .unwrap_or_else(|| Value::Error("MAX of empty range".into()))
        }
        "COUNT" => Value::Num(nums().len() as f64),
        "ABS" => match values.first().and_then(|v| v.as_number()) {
            Some(n) => Value::Num(n.abs()),
            None => Value::Error("ABS expects a number".into()),
        },
        "ROUND" => {
            let n = values.first().and_then(|v| v.as_number());
            let d = values.get(1).and_then(|v| v.as_number()).unwrap_or(0.0);
            match n {
                Some(n) => {
                    let factor = 10f64.powf(d);
                    Value::Num((n * factor).round() / factor)
                }
                None => Value::Error("ROUND expects a number".into()),
            }
        }
        "IF" => {
            // IF(cond, then, else) — non-zero is true
            let c = values.first().and_then(|v| v.as_number()).unwrap_or(0.0);
            if c != 0.0 {
                values.get(1).cloned().unwrap_or(Value::Num(0.0))
            } else {
                values.get(2).cloned().unwrap_or(Value::Num(0.0))
            }
        }
        _ => Value::Error(format!("unknown function: {}", name)),
    }
}

// ============================================================
// Lexer
// ============================================================

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    LParen,
    RParen,
    Comma,
    Colon,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            b')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            b',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            b':' => {
                out.push(Tok::Colon);
                i += 1;
            }
            b'+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            b'-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            b'*' => {
                out.push(Tok::Star);
                i += 1;
            }
            b'/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            b'^' => {
                out.push(Tok::Caret);
                i += 1;
            }
            b'0'..=b'9' | b'.' => {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_digit() || bytes[i] == b'.')
                {
                    i += 1;
                }
                let s = std::str::from_utf8(&bytes[start..i])
                    .map_err(|e| e.to_string())?;
                let n: f64 = s.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
                out.push(Tok::Num(n));
            }
            b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                {
                    i += 1;
                }
                let s = std::str::from_utf8(&bytes[start..i])
                    .map_err(|e| e.to_string())?;
                out.push(Tok::Ident(s.to_string()));
            }
            _ => return Err(format!("unexpected character `{}` in formula", b as char)),
        }
    }
    Ok(out)
}

// ============================================================
// Parser (precedence climbing)
// ============================================================

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_add()
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_mul()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Tok::Plus => Op::Add,
                Tok::Minus => Op::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_mul()?;
            lhs = Expr::BinOp(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_pow()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Tok::Star => Op::Mul,
                Tok::Slash => Op::Div,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_pow()?;
            lhs = Expr::BinOp(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_pow(&mut self) -> Result<Expr, String> {
        let lhs = self.parse_unary()?;
        if matches!(self.peek(), Some(Tok::Caret)) {
            self.bump();
            // right-associative
            let rhs = self.parse_pow()?;
            Ok(Expr::BinOp(Op::Pow, Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if matches!(self.peek(), Some(Tok::Minus)) {
            self.bump();
            let e = self.parse_unary()?;
            Ok(Expr::UnaryMinus(Box::new(e)))
        } else if matches!(self.peek(), Some(Tok::Plus)) {
            self.bump();
            self.parse_unary()
        } else {
            self.parse_atom()
        }
    }

    fn parse_atom(&mut self) -> Result<Expr, String> {
        match self.bump() {
            Some(Tok::Num(n)) => Ok(Expr::Num(n)),
            Some(Tok::LParen) => {
                let e = self.parse_expr()?;
                match self.bump() {
                    Some(Tok::RParen) => Ok(e),
                    other => Err(format!("expected `)`, got {:?}", other)),
                }
            }
            Some(Tok::Ident(name)) => {
                // Either: function call (next is `(`)
                //     or: cell reference (the ident parses as ColLetters[+Digits])
                //     or: range (cell ref ':' cell ref)
                if matches!(self.peek(), Some(Tok::LParen)) {
                    self.bump(); // (
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        loop {
                            // Try to detect ranges before parsing as expression:
                            // a range is `ident colon ident`, both cell refs.
                            args.push(self.parse_arg()?);
                            match self.peek() {
                                Some(Tok::Comma) => {
                                    self.bump();
                                }
                                Some(Tok::RParen) => break,
                                other => {
                                    return Err(format!(
                                        "expected `,` or `)` in arg list, got {:?}",
                                        other
                                    ))
                                }
                            }
                        }
                    }
                    match self.bump() {
                        Some(Tok::RParen) => Ok(Expr::Call(name, args)),
                        other => Err(format!("expected `)` after args, got {:?}", other)),
                    }
                } else if let Some((c, r)) = parse_cell_ident(&name) {
                    Ok(Expr::CellRef(c, r))
                } else {
                    Err(format!("unknown identifier or invalid cell ref: {}", name))
                }
            }
            other => Err(format!("expected atom, got {:?}", other)),
        }
    }

    /// Parse a function argument. Detects `cell:cell` as a Range; falls
    /// back to a regular expression otherwise.
    fn parse_arg(&mut self) -> Result<Expr, String> {
        // Lookahead: ident colon ident → range
        if let (Some(Tok::Ident(a)), Some(Tok::Colon), Some(Tok::Ident(b))) = (
            self.tokens.get(self.pos).cloned(),
            self.tokens.get(self.pos + 1).cloned(),
            self.tokens.get(self.pos + 2).cloned(),
        ) {
            if let (Some((c1, r1)), Some((c2, r2))) =
                (parse_cell_ident(&a), parse_cell_ident(&b))
            {
                self.pos += 3;
                let (lo_c, hi_c) = if c1 <= c2 { (c1, c2) } else { (c2, c1) };
                let (lo_r, hi_r) = if r1 <= r2 { (r1, r2) } else { (r2, r1) };
                return Ok(Expr::Range((lo_c, lo_r), (hi_c, hi_r)));
            }
        }
        self.parse_expr()
    }
}

/// Parse a cell-reference identifier ("A1", "AB12") into (col, row) 0-based.
/// Returns None for plain function names ("SUM", "AVG") or column-only ("A").
fn parse_cell_ident(s: &str) -> Option<(u32, u32)> {
    if s.is_empty() {
        return None;
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == 0 || i == bytes.len() {
        return None;
    }
    let letters = &s[..i];
    let digits = &s[i..];
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut col: u32 = 0;
    for c in letters.chars() {
        let v = (c.to_ascii_uppercase() as u32) - (b'A' as u32) + 1;
        col = col.checked_mul(26)?.checked_add(v)?;
    }
    let row: u32 = digits.parse().ok()?;
    if row == 0 {
        return None;
    }
    Some((col - 1, row - 1))
}

// ============================================================
// Dependency-resolving evaluator for sheets
// ============================================================

/// Evaluate every formula cell in a sheet, returning a map from
/// (col, row) → resolved `Value`. Cells without formulas (raw text or
/// numbers) get stored as `Num(n)` if parseable, else `Str(text)`.
///
/// Detects cycles and marks every cell in a cycle as `Value::Error`.
pub fn evaluate_sheet<F>(cells: &HashMap<(u32, u32), String>) -> HashMap<(u32, u32), Value>
where
    F: Fn(u32, u32) -> Option<String>,
{
    // Parse formulas up front; non-formula cells stay as text.
    enum Slot {
        Literal(Value),
        Formula(Expr),
    }
    let mut slots: HashMap<(u32, u32), Slot> = HashMap::new();
    for (&addr, body) in cells {
        if let Some(rest) = body.strip_prefix('=') {
            match parse_formula(rest) {
                Ok(expr) => {
                    slots.insert(addr, Slot::Formula(expr));
                }
                Err(e) => {
                    slots.insert(addr, Slot::Literal(Value::Error(format!("parse: {}", e))));
                }
            }
        } else if let Ok(n) = body.trim().parse::<f64>() {
            slots.insert(addr, Slot::Literal(Value::Num(n)));
        } else {
            slots.insert(addr, Slot::Literal(Value::Str(body.clone())));
        }
    }

    // Memo + cycle detection
    let mut memo: HashMap<(u32, u32), Value> = HashMap::new();
    let mut in_progress: HashSet<(u32, u32)> = HashSet::new();

    fn resolve(
        addr: (u32, u32),
        slots: &HashMap<(u32, u32), Slot>,
        memo: &mut HashMap<(u32, u32), Value>,
        in_progress: &mut HashSet<(u32, u32)>,
    ) -> Value {
        if let Some(v) = memo.get(&addr) {
            return v.clone();
        }
        if in_progress.contains(&addr) {
            return Value::Error(format!("cycle at {}", format_addr(addr.0, addr.1)));
        }
        let slot = match slots.get(&addr) {
            Some(s) => s,
            None => return Value::Num(0.0), // empty cell
        };
        in_progress.insert(addr);
        let value = match slot {
            Slot::Literal(v) => v.clone(),
            Slot::Formula(expr) => {
                // Build a one-shot env that resolves recursively.
                struct Env<'a> {
                    slots: &'a HashMap<(u32, u32), Slot>,
                    memo: std::cell::RefCell<&'a mut HashMap<(u32, u32), Value>>,
                    in_progress: std::cell::RefCell<&'a mut HashSet<(u32, u32)>>,
                }
                impl<'a> CellEnv for Env<'a> {
                    fn get(&self, c: u32, r: u32) -> Value {
                        let mut memo = self.memo.borrow_mut();
                        let mut ip = self.in_progress.borrow_mut();
                        resolve((c, r), self.slots, &mut memo, &mut ip)
                    }
                }
                let env = Env {
                    slots,
                    memo: std::cell::RefCell::new(memo),
                    in_progress: std::cell::RefCell::new(in_progress),
                };
                let v = eval(expr, &env);
                drop(env);
                v
            }
        };
        in_progress.remove(&addr);
        memo.insert(addr, value.clone());
        value
    }

    let addrs: Vec<_> = slots.keys().copied().collect();
    for addr in addrs {
        let _ = resolve(addr, &slots, &mut memo, &mut in_progress);
    }
    let _ = std::marker::PhantomData::<F>;
    memo
}

fn format_addr(col: u32, row: u32) -> String {
    let mut letters = String::new();
    let mut n = col + 1;
    while n > 0 {
        let r = (n - 1) % 26;
        letters.insert(0, (b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    format!("{}{}", letters, row + 1)
}

/// Format a `Value` for display, applying an optional sheet `fmt`
/// hint (`number`/`currency`/`percent`/`date`/`datetime`/`text`).
pub fn format_value(value: &Value, fmt: Option<&str>) -> String {
    match value {
        Value::Num(n) => format_number(*n, fmt),
        Value::Str(s) => s.clone(),
        Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Value::Error(e) => format!("#ERROR: {}", e),
    }
}

fn format_number(n: f64, fmt: Option<&str>) -> String {
    match fmt.unwrap_or("number") {
        "currency" => format_grouped(n, 2, "$"),
        "percent" => format!("{:.1}%", n * 100.0),
        "number" | _ => format_grouped(n, 2, ""),
    }
}

fn format_grouped(n: f64, decimals: usize, prefix: &str) -> String {
    let sign = if n < 0.0 { "-" } else { "" };
    let abs = n.abs();
    let int_part = abs.trunc() as u64;
    let frac_part = abs - abs.trunc();

    // Group thousands
    let int_str = group_thousands(int_part);
    if decimals == 0 {
        return format!("{}{}{}", sign, prefix, int_str);
    }
    let frac_str = format!("{:.*}", decimals, frac_part);
    let frac_digits = &frac_str[2..]; // drop "0."
    format!("{}{}{}.{}", sign, prefix, int_str, frac_digits)
}

fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticEnv(HashMap<(u32, u32), f64>);
    impl CellEnv for StaticEnv {
        fn get(&self, c: u32, r: u32) -> Value {
            self.0
                .get(&(c, r))
                .copied()
                .map(Value::Num)
                .unwrap_or(Value::Num(0.0))
        }
    }

    fn env(cells: &[(u32, u32, f64)]) -> StaticEnv {
        let mut m = HashMap::new();
        for (c, r, v) in cells {
            m.insert((*c, *r), *v);
        }
        StaticEnv(m)
    }

    fn ev(src: &str, e: &StaticEnv) -> Value {
        let expr = parse_formula(src).expect("parse");
        eval(&expr, e)
    }

    #[test]
    fn arithmetic_basic() {
        let e = env(&[]);
        assert_eq!(ev("=1+2", &e), Value::Num(3.0));
        assert_eq!(ev("=10-3", &e), Value::Num(7.0));
        assert_eq!(ev("=2*3", &e), Value::Num(6.0));
        assert_eq!(ev("=10/4", &e), Value::Num(2.5));
        assert_eq!(ev("=2^3", &e), Value::Num(8.0));
    }

    #[test]
    fn arithmetic_precedence() {
        let e = env(&[]);
        assert_eq!(ev("=1+2*3", &e), Value::Num(7.0));
        assert_eq!(ev("=(1+2)*3", &e), Value::Num(9.0));
        assert_eq!(ev("=2^3^2", &e), Value::Num(512.0)); // right-assoc: 2^(3^2)
    }

    #[test]
    fn unary_minus() {
        let e = env(&[]);
        assert_eq!(ev("=-5", &e), Value::Num(-5.0));
        assert_eq!(ev("=-(2+3)", &e), Value::Num(-5.0));
        assert_eq!(ev("=10+-3", &e), Value::Num(7.0));
    }

    #[test]
    fn cell_ref_lookup() {
        let e = env(&[(0, 0, 100.0)]); // A1 = 100
        assert_eq!(ev("=A1", &e), Value::Num(100.0));
        assert_eq!(ev("=A1*2", &e), Value::Num(200.0));
    }

    #[test]
    fn sum_range() {
        let e = env(&[(0, 0, 1.0), (0, 1, 2.0), (0, 2, 3.0)]); // A1=1 A2=2 A3=3
        assert_eq!(ev("=SUM(A1:A3)", &e), Value::Num(6.0));
    }

    #[test]
    fn avg_min_max_count() {
        let e = env(&[(0, 0, 10.0), (0, 1, 20.0), (0, 2, 30.0)]);
        assert_eq!(ev("=AVERAGE(A1:A3)", &e), Value::Num(20.0));
        assert_eq!(ev("=AVG(A1:A3)", &e), Value::Num(20.0));
        assert_eq!(ev("=MIN(A1:A3)", &e), Value::Num(10.0));
        assert_eq!(ev("=MAX(A1:A3)", &e), Value::Num(30.0));
        assert_eq!(ev("=COUNT(A1:A3)", &e), Value::Num(3.0));
    }

    #[test]
    fn division_by_zero_errors() {
        let e = env(&[]);
        assert!(ev("=10/0", &e).is_error());
    }

    #[test]
    fn unknown_function_errors() {
        let e = env(&[]);
        assert!(ev("=BOGUS(1, 2)", &e).is_error());
    }

    #[test]
    fn abs_and_round() {
        let e = env(&[]);
        assert_eq!(ev("=ABS(-7)", &e), Value::Num(7.0));
        assert_eq!(ev("=ROUND(3.14159, 2)", &e), Value::Num(3.14));
        assert_eq!(ev("=ROUND(2.5, 0)", &e), Value::Num(3.0));
    }

    #[test]
    fn if_function() {
        let e = env(&[]);
        assert_eq!(ev("=IF(1, 10, 20)", &e), Value::Num(10.0));
        assert_eq!(ev("=IF(0, 10, 20)", &e), Value::Num(20.0));
    }

    #[test]
    fn cell_parse_ident() {
        assert_eq!(parse_cell_ident("A1"), Some((0, 0)));
        assert_eq!(parse_cell_ident("B5"), Some((1, 4)));
        assert_eq!(parse_cell_ident("AA1"), Some((26, 0)));
        assert_eq!(parse_cell_ident("SUM"), None); // no digits
        assert_eq!(parse_cell_ident("123"), None); // no letters
        assert_eq!(parse_cell_ident("A0"), None); // row must be >= 1
    }

    #[test]
    fn format_currency() {
        assert_eq!(format_value(&Value::Num(1250000.0), Some("currency")), "$1,250,000.00");
        assert_eq!(format_value(&Value::Num(-82000.0), Some("currency")), "-$82,000.00");
        assert_eq!(format_value(&Value::Num(42.5), Some("currency")), "$42.50");
    }

    #[test]
    fn format_percent() {
        assert_eq!(format_value(&Value::Num(0.35), Some("percent")), "35.0%");
        assert_eq!(format_value(&Value::Num(1.03), Some("percent")), "103.0%");
    }

    #[test]
    fn evaluate_sheet_solves_chain() {
        // A1 = 10, A2 = 20, A3 = =SUM(A1:A2), A4 = =A3*2
        let mut cells = HashMap::new();
        cells.insert((0, 0), "10".to_string());
        cells.insert((0, 1), "20".to_string());
        cells.insert((0, 2), "=SUM(A1:A2)".to_string());
        cells.insert((0, 3), "=A3*2".to_string());
        let r = evaluate_sheet::<fn(u32, u32) -> Option<String>>(&cells);
        assert_eq!(r.get(&(0, 2)), Some(&Value::Num(30.0)));
        assert_eq!(r.get(&(0, 3)), Some(&Value::Num(60.0)));
    }

    #[test]
    fn evaluate_sheet_detects_cycle() {
        // A1 = =A2, A2 = =A1
        let mut cells = HashMap::new();
        cells.insert((0, 0), "=A2".to_string());
        cells.insert((0, 1), "=A1".to_string());
        let r = evaluate_sheet::<fn(u32, u32) -> Option<String>>(&cells);
        assert!(r.get(&(0, 0)).unwrap().is_error());
        assert!(r.get(&(0, 1)).unwrap().is_error());
    }
}
