//! Shared property / unit helpers used by both the docx and html
//! renderers.
//!
//! Both accept the same unit set on length/spacing properties
//! (`Npt`, `Nin`, `Ncm`, `Nmm`, `Npx`; bare = pt), but convert to
//! format-specific units (`dxa` for docx, `pt`/`px` for CSS). The
//! point-valued shared helpers live here so the accepted surface
//! stays in sync.

/// Parse a length string to points. Accepts `Npt`, `Nin`, `Ncm`,
/// `Nmm`, `Npx` (96 dpi), bare = pt. Returns `None` for
/// unrecognized units or negatives.
pub fn parse_length_to_points(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let idx = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
    let (num, unit) = s.split_at(idx);
    let value: f64 = num.parse().ok()?;
    let pts = match unit {
        "" | "pt" => value,
        "in" => value * 72.0,
        "cm" => value * 28.3464566929,
        "mm" => value * 2.83464566929,
        "px" => value * 0.75,
        _ => return None,
    };
    if pts < 0.0 {
        return None;
    }
    Some(pts)
}

/// Line-height value parsed from a `line:` property — either an
/// explicit point value (`line:18pt`) or a unitless multiplier
/// (`line:1.5x`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    /// Multiplier of the single-line baseline.
    Multiple(f64),
    /// Absolute height in points.
    Points(f64),
}

pub fn parse_line(s: &str) -> Option<LineHeight> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('x') {
        let mult: f64 = num.parse().ok()?;
        if mult <= 0.0 {
            return None;
        }
        return Some(LineHeight::Multiple(mult));
    }
    parse_length_to_points(s).map(LineHeight::Points)
}

/// Map Stem's alignment vocabulary to a CSS `text-align` keyword.
/// Returns `None` for values the spec doesn't recognize so callers
/// can ignore unknowns rather than emit garbage CSS.
pub fn map_align_css(s: &str) -> Option<&'static str> {
    match s {
        "left" => Some("left"),
        "right" => Some("right"),
        "center" | "centre" => Some("center"),
        "justify" | "both" => Some("justify"),
        _ => None,
    }
}

/// Normalize an authored hex color (`"#abc123"` or `"abc123"`) to
/// the 6-character uppercase form. Returns `None` for any other
/// shape — short `#abc` notation is not accepted because the docx
/// side also rejects it (OOXML requires 6 hex).
pub fn normalize_hex_color(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches('#');
    if t.len() == 6 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(t.to_uppercase())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_units_round_to_points() {
        assert_eq!(parse_length_to_points("12pt"), Some(12.0));
        assert_eq!(parse_length_to_points("12"), Some(12.0));
        assert_eq!(parse_length_to_points("1in"), Some(72.0));
        assert_eq!(parse_length_to_points("2.54cm").map(|v| v.round()), Some(72.0));
        assert_eq!(parse_length_to_points("25.4mm").map(|v| v.round()), Some(72.0));
        assert_eq!(parse_length_to_points("96px"), Some(72.0));
        assert_eq!(parse_length_to_points("xyz"), None);
        assert_eq!(parse_length_to_points("-1pt"), None);
    }

    #[test]
    fn line_height_multiplier_vs_points() {
        assert_eq!(parse_line("1.5x"), Some(LineHeight::Multiple(1.5)));
        assert_eq!(parse_line("18pt"), Some(LineHeight::Points(18.0)));
        assert_eq!(parse_line("0x"), None);
    }

    #[test]
    fn align_maps_centre_and_both() {
        assert_eq!(map_align_css("centre"), Some("center"));
        assert_eq!(map_align_css("both"), Some("justify"));
        assert_eq!(map_align_css("zz"), None);
    }

    #[test]
    fn hex_color_uppercases_and_validates() {
        assert_eq!(normalize_hex_color("#abc123"), Some("ABC123".to_string()));
        assert_eq!(normalize_hex_color("ABC123"), Some("ABC123".to_string()));
        assert_eq!(normalize_hex_color("#abc"), None);
        assert_eq!(normalize_hex_color("zzzzzz"), None);
    }
}
