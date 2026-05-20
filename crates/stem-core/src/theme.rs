//! Theme types — small, stable, format-agnostic. Renderers map these
//! names to format-native concepts (hex codes, named styles, etc.).

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub colors: BTreeMap<String, ThemeColor>,
    pub fonts: ThemeFonts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ThemeColor {
    /// RGB triple in the sRGB color space.
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ThemeColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeFonts {
    pub body: String,
    pub heading: String,
    pub mono: String,
}

impl Default for ThemeFonts {
    fn default() -> Self {
        Self {
            body: "system-ui, -apple-system, Segoe UI, sans-serif".into(),
            heading: "system-ui, -apple-system, Segoe UI, sans-serif".into(),
            mono: "ui-monospace, SFMono-Regular, Menlo, monospace".into(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        let mut colors = BTreeMap::new();
        colors.insert("primary".into(), ThemeColor::rgb(0x1f, 0x6f, 0xeb));
        colors.insert("text".into(), ThemeColor::rgb(0x14, 0x18, 0x1f));
        colors.insert("muted".into(), ThemeColor::rgb(0x6e, 0x77, 0x81));
        colors.insert("background".into(), ThemeColor::rgb(0xff, 0xff, 0xff));
        colors.insert("rule".into(), ThemeColor::rgb(0xd0, 0xd7, 0xde));
        colors.insert("red".into(), ThemeColor::rgb(0xcf, 0x22, 0x2e));
        colors.insert("yellow".into(), ThemeColor::rgb(0xff, 0xd3, 0x3d));
        colors.insert("gray".into(), ThemeColor::rgb(0xf6, 0xf8, 0xfa));
        Self {
            name: "default".into(),
            colors,
            fonts: ThemeFonts::default(),
        }
    }
}

impl Theme {
    /// Resolve a color name, falling back to literal hex (`#rrggbb`) if the
    /// name isn't registered.
    pub fn resolve_color(&self, name: &str) -> Option<ThemeColor> {
        if let Some(c) = self.colors.get(name) {
            return Some(*c);
        }
        parse_hex(name)
    }
}

fn parse_hex(s: &str) -> Option<ThemeColor> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(ThemeColor::rgb(r, g, b))
}
