//! LaTeX → MathML conversion for `@math` rendering.
//!
//! Uses `pulldown-latex` to parse and emit MathML. The HTML renderer's
//! `math` element module calls into this; other backends (docx, pdf)
//! will get their own conversion paths.

use pulldown_latex::config::{DisplayMode, RenderConfig};
use pulldown_latex::mathml::push_mathml;
use pulldown_latex::{Parser, Storage};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MathError {
    #[error("latex: {0}")]
    Latex(String),
    #[error("unsupported notation: {0}")]
    UnsupportedNotation(String),
}

/// Convert a math expression in the given notation to MathML.
///
/// `notation` is one of `"latex"`, `"asciimath"`, `"mathml"`. Only
/// `latex` and `mathml` (pass-through) are implemented today.
pub fn to_mathml(src: &str, notation: &str, block: bool) -> Result<String, MathError> {
    match notation {
        "latex" => latex_to_mathml(src, block),
        "mathml" => Ok(src.trim().to_string()),
        other => Err(MathError::UnsupportedNotation(other.to_string())),
    }
}

fn latex_to_mathml(src: &str, block: bool) -> Result<String, MathError> {
    let storage = Storage::new();
    let parser = Parser::new(src, &storage);
    let mut out = String::new();
    let config = RenderConfig {
        display_mode: if block { DisplayMode::Block } else { DisplayMode::Inline },
        ..Default::default()
    };
    push_mathml(&mut out, parser, config).map_err(|e| MathError::Latex(e.to_string()))?;
    Ok(out)
}

/// Cheap syntactic check used by validate-time. Returns `Err` for
/// LaTeX inputs that fail to parse. `mathml` and unknown notations
/// return `Ok` (the renderer surfaces those separately).
pub fn check(src: &str, notation: &str) -> Result<(), MathError> {
    if notation != "latex" {
        return Ok(());
    }
    // Parser is an iterator of Results; drive it to completion and
    // surface the first error.
    let storage = Storage::new();
    let parser = Parser::new(src, &storage);
    for event in parser {
        if let Err(e) = event {
            return Err(MathError::Latex(e.to_string()));
        }
    }
    Ok(())
}
