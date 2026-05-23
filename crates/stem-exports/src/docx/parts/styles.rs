//! `word/styles.xml` — paragraph + character style set referenced
//! by document body and TOC/LoT/LoF emission.
//!
//! The reason we left docx-rs: it interleaved `<w:style>` children
//! (uiPriority/qFormat after pPr/rPr) and `<w:pPr>` children
//! (rPr before pStyle), which made Word's TOC scan and Heading
//! rendering misbehave. This module emits children in strict
//! schema order on the first pass — the order is encoded in the
//! `Style` builder's `render` method, not in caller code.
//!
//! Schema order summary (CT_Style):
//!   name → basedOn → next → link → autoRedefine → hidden →
//!   uiPriority → semiHidden → unhideWhenUsed → qFormat → locked →
//!   personal{,Compose,Reply} → rsid → pPr → rPr → tblPr → trPr →
//!   tcPr → tblStylePr
//!
//! Within `<w:pPr>` (the slots we use):
//!   keepNext → keepLines → numPr → spacing → ind → jc → outlineLvl
//!
//! Within `<w:rPr>` (the slots we use):
//!   rFonts → b → bCs → i → iCs → strike → color → sz → szCs →
//!   u → vertAlign

use super::super::xml::XmlBuf;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Run properties — character-level formatting. Used inside a
/// style's `<w:rPr>` and inside the paragraph properties when a
/// style mark also needs run formatting (the "paragraph mark"
/// `<w:rPr>` nested inside `<w:pPr>`).
#[derive(Default, Clone)]
pub struct RPr {
    pub fonts: Option<Fonts>,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    /// "auto" or 6-hex RGB ("0563C1").
    pub color: Option<String>,
    /// Half-points.
    pub size_hp: Option<u32>,
    /// "single", "double", "none", ...
    pub underline: Option<String>,
    /// "superscript", "subscript", or None for baseline.
    pub vert_align: Option<String>,
}

#[derive(Default, Clone)]
pub struct Fonts {
    pub ascii: Option<String>,
    pub hi_ansi: Option<String>,
    /// Theme alias — e.g. "majorHAnsi" or "minorHAnsi". When set,
    /// overrides the literal `ascii`/`hi_ansi` if a reader resolves
    /// themes.
    pub ascii_theme: Option<String>,
    pub h_ansi_theme: Option<String>,
    pub east_asia_theme: Option<String>,
    pub cs_theme: Option<String>,
}

/// Paragraph properties — block-level formatting. Used inside a
/// style's `<w:pPr>`.
#[derive(Default, Clone)]
pub struct PPr {
    pub keep_next: bool,
    pub keep_lines: bool,
    /// (before, after, line) all in twentieths of a point. `line`
    /// is paired with line_rule "auto".
    pub spacing: Option<Spacing>,
    /// (left, hanging) — left indent and optional hanging indent
    /// in twentieths of a point.
    pub ind: Option<Indent>,
    /// "left", "center", "right", "both".
    pub jc: Option<String>,
    /// 0..=8 for Heading1..Heading9.
    pub outline_lvl: Option<u32>,
    /// Contextual spacing — collapse "after" spacing when this and
    /// the next paragraph share a style. Used on ListParagraph.
    pub contextual_spacing: bool,
    /// When true, the style's pPr will emit a single-line `<w:pBdr>`
    /// with a top border. Set via `style[id:..., border-top:true]`.
    pub border_top: bool,
}

#[derive(Default, Clone, Copy)]
pub struct Spacing {
    pub before: Option<u32>,
    pub after: Option<u32>,
    pub line: Option<u32>,
}

#[derive(Default, Clone, Copy)]
pub struct Indent {
    pub left: Option<u32>,
    pub hanging: Option<u32>,
    pub first_line: Option<u32>,
}

#[derive(Clone, Copy)]
pub enum StyleType {
    Paragraph,
    Character,
    Table,
    Numbering,
}

impl StyleType {
    fn attr(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Character => "character",
            Self::Table => "table",
            Self::Numbering => "numbering",
        }
    }
}

/// One style entry. Construct with [`Style::paragraph`] /
/// [`Style::character`] etc., chain setters, then call `render` to
/// append to an `XmlBuf`.
///
/// `id` and `name` are owned `String` so callers can pass either
/// `"Heading1"` or a computed `format!("toc {n}")` without lifetime
/// gymnastics. Allocations are cheap relative to ZIP/XML work.
pub struct Style {
    pub kind: StyleType,
    pub id: String,
    pub name: String,
    pub default: bool,
    pub custom: bool,
    pub based_on: Option<String>,
    pub next: Option<String>,
    pub link: Option<String>,
    pub ui_priority: Option<u32>,
    pub semi_hidden: bool,
    pub unhide_when_used: bool,
    pub q_format: bool,
    pub p_pr: Option<PPr>,
    pub r_pr: Option<RPr>,
}

impl Style {
    fn new(kind: StyleType, id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
            name: name.into(),
            default: false,
            custom: false,
            based_on: None,
            next: None,
            link: None,
            ui_priority: None,
            semi_hidden: false,
            unhide_when_used: false,
            q_format: false,
            p_pr: None,
            r_pr: None,
        }
    }

    pub fn paragraph(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(StyleType::Paragraph, id, name)
    }
    pub fn character(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(StyleType::Character, id, name)
    }
    pub fn table(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(StyleType::Table, id, name)
    }
    pub fn numbering(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(StyleType::Numbering, id, name)
    }

    pub fn default(mut self) -> Self {
        self.default = true;
        self
    }
    pub fn custom(mut self) -> Self {
        self.custom = true;
        self
    }
    pub fn based_on(mut self, id: impl Into<String>) -> Self {
        self.based_on = Some(id.into());
        self
    }
    pub fn next(mut self, id: impl Into<String>) -> Self {
        self.next = Some(id.into());
        self
    }
    pub fn link(mut self, id: impl Into<String>) -> Self {
        self.link = Some(id.into());
        self
    }
    pub fn ui_priority(mut self, p: u32) -> Self {
        self.ui_priority = Some(p);
        self
    }
    pub fn semi_hidden(mut self) -> Self {
        self.semi_hidden = true;
        self
    }
    pub fn unhide_when_used(mut self) -> Self {
        self.unhide_when_used = true;
        self
    }
    pub fn q_format(mut self) -> Self {
        self.q_format = true;
        self
    }
    pub fn p_pr(mut self, p: PPr) -> Self {
        self.p_pr = Some(p);
        self
    }
    pub fn r_pr(mut self, r: RPr) -> Self {
        self.r_pr = Some(r);
        self
    }

    /// Render this style, applying any matching source-supplied
    /// override on top of the built-in defaults first.
    pub fn render_with(
        self,
        x: &mut XmlBuf,
        overrides: &[super::super::emit::ctx::StyleOverride],
    ) {
        apply_overrides(self, overrides).render(x);
    }

    pub fn render(&self, x: &mut XmlBuf) {
        let mut attrs: Vec<(&str, &str)> = vec![("w:type", self.kind.attr())];
        if self.default {
            attrs.push(("w:default", "1"));
        }
        if self.custom {
            attrs.push(("w:customStyle", "1"));
        }
        attrs.push(("w:styleId", &self.id));
        x.elem("w:style", &attrs, |x| {
            // 1. name
            x.empty("w:name", &[("w:val", &self.name)]);
            // 2. basedOn
            if let Some(b) = &self.based_on {
                x.empty("w:basedOn", &[("w:val", b.as_str())]);
            }
            // 3. next
            if let Some(n) = &self.next {
                x.empty("w:next", &[("w:val", n.as_str())]);
            }
            // 4. link
            if let Some(l) = &self.link {
                x.empty("w:link", &[("w:val", l.as_str())]);
            }
            // 5. uiPriority
            if let Some(p) = self.ui_priority {
                let s = p.to_string();
                x.empty("w:uiPriority", &[("w:val", &s)]);
            }
            // 6. semiHidden
            if self.semi_hidden {
                x.empty("w:semiHidden", &[]);
            }
            // 7. unhideWhenUsed
            if self.unhide_when_used {
                x.empty("w:unhideWhenUsed", &[]);
            }
            // 8. qFormat
            if self.q_format {
                x.empty("w:qFormat", &[]);
            }
            // 9. pPr
            if let Some(p) = &self.p_pr {
                render_p_pr(x, p);
            }
            // 10. rPr
            if let Some(r) = &self.r_pr {
                render_r_pr(x, r);
            }
        });
    }
}

fn render_p_pr(x: &mut XmlBuf, p: &PPr) {
    x.elem("w:pPr", &[], |x| {
        if p.keep_next {
            x.empty("w:keepNext", &[]);
        }
        if p.keep_lines {
            x.empty("w:keepLines", &[]);
        }
        if p.border_top {
            x.elem("w:pBdr", &[], |x| {
                x.empty(
                    "w:top",
                    &[
                        ("w:val", "single"),
                        ("w:sz", "4"),
                        ("w:space", "1"),
                        ("w:color", "auto"),
                    ],
                );
            });
        }
        if let Some(s) = p.spacing {
            let mut attrs: Vec<(&str, String)> = Vec::with_capacity(4);
            if let Some(v) = s.before {
                attrs.push(("w:before", v.to_string()));
            }
            if let Some(v) = s.after {
                attrs.push(("w:after", v.to_string()));
            }
            if let Some(v) = s.line {
                attrs.push(("w:line", v.to_string()));
                attrs.push(("w:lineRule", "auto".to_string()));
            }
            let refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
            x.empty("w:spacing", &refs);
        }
        if let Some(i) = p.ind {
            let mut attrs: Vec<(&str, String)> = Vec::with_capacity(3);
            if let Some(v) = i.left {
                attrs.push(("w:left", v.to_string()));
            }
            if let Some(v) = i.hanging {
                attrs.push(("w:hanging", v.to_string()));
            }
            if let Some(v) = i.first_line {
                attrs.push(("w:firstLine", v.to_string()));
            }
            let refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
            x.empty("w:ind", &refs);
        }
        if p.contextual_spacing {
            x.empty("w:contextualSpacing", &[]);
        }
        if let Some(j) = &p.jc {
            x.empty("w:jc", &[("w:val", j.as_str())]);
        }
        if let Some(o) = p.outline_lvl {
            let s = o.to_string();
            x.empty("w:outlineLvl", &[("w:val", &s)]);
        }
    });
}

fn render_r_pr(x: &mut XmlBuf, r: &RPr) {
    x.elem("w:rPr", &[], |x| {
        if let Some(f) = &r.fonts {
            let mut attrs: Vec<(&str, &str)> = Vec::new();
            if let Some(s) = &f.ascii {
                attrs.push(("w:ascii", s.as_str()));
            }
            if let Some(s) = &f.hi_ansi {
                attrs.push(("w:hAnsi", s.as_str()));
            }
            if let Some(s) = &f.ascii_theme {
                attrs.push(("w:asciiTheme", s.as_str()));
            }
            if let Some(s) = &f.h_ansi_theme {
                attrs.push(("w:hAnsiTheme", s.as_str()));
            }
            if let Some(s) = &f.east_asia_theme {
                attrs.push(("w:eastAsiaTheme", s.as_str()));
            }
            if let Some(s) = &f.cs_theme {
                attrs.push(("w:cstheme", s.as_str()));
            }
            x.empty("w:rFonts", &attrs);
        }
        if r.bold {
            x.empty("w:b", &[]);
            x.empty("w:bCs", &[]);
        }
        if r.italic {
            x.empty("w:i", &[]);
            x.empty("w:iCs", &[]);
        }
        if r.strike {
            x.empty("w:strike", &[]);
        }
        if let Some(c) = &r.color {
            x.empty("w:color", &[("w:val", c.as_str())]);
        }
        if let Some(sz) = r.size_hp {
            let s = sz.to_string();
            x.empty("w:sz", &[("w:val", &s)]);
            x.empty("w:szCs", &[("w:val", &s)]);
        }
        if let Some(u) = &r.underline {
            x.empty("w:u", &[("w:val", u.as_str())]);
        }
        if let Some(v) = &r.vert_align {
            x.empty("w:vertAlign", &[("w:val", v.as_str())]);
        }
    });
}

/// Build the complete `styles.xml` part.
///
/// Layout:
///   docDefaults (rPrDefault: Calibri 11pt, Latin/EastAsia/CS;
///                pPrDefault: 8pt-after-160, 1.08× line)
///   latentStyles (terse — the qFormat exceptions we depend on)
///   real styles: Normal, DefaultParagraphFont, TableNormal,
///                NoList, Heading1..6, Title, Caption, Hyperlink,
///                FootnoteReference, TOC1..9, TOCHeading,
///                TableofFigures, ListParagraph
pub fn styles() -> String {
    styles_with_overrides(&[])
}

pub fn styles_with_overrides(
    overrides: &[super::super::emit::ctx::StyleOverride],
) -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem("w:styles", &[("xmlns:w", NS_W)], |x| {
        // docDefaults — required by the style schema; Word also
        // reads these when no override style applies.
        x.elem("w:docDefaults", &[], |x| {
            x.elem("w:rPrDefault", &[], |x| {
                x.elem("w:rPr", &[], |x| {
                    x.empty(
                        "w:rFonts",
                        &[
                            ("w:asciiTheme", "minorHAnsi"),
                            ("w:eastAsiaTheme", "minorHAnsi"),
                            ("w:hAnsiTheme", "minorHAnsi"),
                            ("w:cstheme", "minorBidi"),
                        ],
                    );
                    x.empty("w:sz", &[("w:val", "22")]);
                    x.empty("w:szCs", &[("w:val", "22")]);
                });
            });
            x.elem("w:pPrDefault", &[], |x| {
                x.elem("w:pPr", &[], |x| {
                    x.empty(
                        "w:spacing",
                        &[
                            ("w:after", "160"),
                            ("w:line", "259"),
                            ("w:lineRule", "auto"),
                        ],
                    );
                });
            });
        });

        // latentStyles — just the exceptions Word's heading list
        // and TOC field rely on. defLockedState/defUIPriority/
        // defSemiHidden/defUnhideWhenUsed/defQFormat are the
        // canonical defaults from a fresh Word doc; count="375"
        // matches the size of Word's built-in latent style list.
        x.elem(
            "w:latentStyles",
            &[
                ("w:defLockedState", "0"),
                ("w:defUIPriority", "99"),
                ("w:defSemiHidden", "0"),
                ("w:defUnhideWhenUsed", "0"),
                ("w:defQFormat", "0"),
                ("w:count", "375"),
            ],
            |x| {
                // qFormat-on exceptions for the user-visible styles.
                latent_qformat(x, "Normal", Some(0));
                for level in 1..=6 {
                    let name = format!("heading {level}");
                    latent_qformat(x, &name, Some(9));
                }
                latent_qformat(x, "Title", Some(10));
                latent_qformat(x, "Subtitle", Some(11));
                latent_qformat(x, "Strong", Some(22));
                latent_qformat(x, "Emphasis", Some(20));
                latent_qformat(x, "caption", Some(35));
                latent_qformat(x, "List Paragraph", Some(34));
                // TOC styles need uiPriority 39 so they're visible
                // in Word's "Apply Styles" panel after TOC
                // generation.
                for n in 1..=9 {
                    let name = format!("toc {n}");
                    latent_uipri(x, &name, 39);
                }
            },
        );

        render_real_styles(x, overrides);
    });
    x.finish()
}

fn latent_qformat(x: &mut XmlBuf, name: &str, ui_priority: Option<u32>) {
    let priority_str = ui_priority.map(|p| p.to_string());
    let mut attrs: Vec<(&str, &str)> = vec![("w:name", name)];
    if let Some(p) = &priority_str {
        attrs.push(("w:uiPriority", p.as_str()));
    }
    attrs.push(("w:qFormat", "1"));
    x.empty("w:lsdException", &attrs);
}

fn latent_uipri(x: &mut XmlBuf, name: &str, priority: u32) {
    let p = priority.to_string();
    x.empty(
        "w:lsdException",
        &[("w:name", name), ("w:uiPriority", p.as_str())],
    );
}

fn apply_overrides(style: Style, overrides: &[super::super::emit::ctx::StyleOverride]) -> Style {
    let mut s = style;
    let Some(o) = overrides.iter().find(|o| o.id == s.id) else {
        return s;
    };
    // Build pPr/rPr if missing so we can patch.
    let mut p = s.p_pr.take().unwrap_or_default();
    let mut r = s.r_pr.take().unwrap_or_default();
    // pPr overrides
    let mut spacing = p.spacing.unwrap_or_default();
    let mut p_touched = false;
    if let Some(v) = o.before_dxa {
        spacing.before = Some(v);
        p_touched = true;
    }
    if let Some(v) = o.after_dxa {
        spacing.after = Some(v);
        p_touched = true;
    }
    if let Some(v) = o.line_dxa {
        spacing.line = Some(v);
        p_touched = true;
    }
    if p_touched {
        p.spacing = Some(spacing);
    }
    if let Some(v) = &o.align {
        p.jc = Some(v.clone());
    }
    if let Some(v) = o.keep_next {
        p.keep_next = v;
    }
    if let Some(v) = o.keep_lines {
        p.keep_lines = v;
    }
    if let Some(v) = o.outline_lvl {
        p.outline_lvl = Some(v);
    }
    if let Some(v) = o.contextual_spacing {
        p.contextual_spacing = v;
    }
    if let Some(v) = o.border_top {
        p.border_top = v;
    }
    s.p_pr = Some(p);
    // rPr overrides
    if let Some(v) = o.size_hp {
        r.size_hp = Some(v);
    }
    if let Some(v) = &o.color {
        r.color = Some(v.clone());
    }
    if let Some(v) = o.bold {
        r.bold = v;
    }
    if let Some(v) = o.italic {
        r.italic = v;
    }
    if let Some(v) = o.strike {
        r.strike = v;
    }
    if let Some(v) = &o.underline {
        r.underline = Some(v.clone());
    }
    if let Some(v) = &o.font {
        // Setting `font:` clears any theme alias and forces ascii/hAnsi
        // to the literal family name — matches Word's behavior when
        // you change a style's font from a theme to a specific face.
        r.fonts = Some(Fonts {
            ascii: Some(v.clone()),
            hi_ansi: Some(v.clone()),
            ..Default::default()
        });
    }
    s.r_pr = Some(r);
    s
}

fn render_real_styles(x: &mut XmlBuf, overrides: &[super::super::emit::ctx::StyleOverride]) {
    // Word's three built-in defaults that every docx needs.
    Style::paragraph("Normal", "Normal").default().q_format().render_with(x, overrides);
    Style::character("DefaultParagraphFont", "Default Paragraph Font")
        .default()
        .ui_priority(1)
        .semi_hidden()
        .unhide_when_used()
        .render_with(x, overrides);
    Style::table("TableNormal", "Normal Table")
        .default()
        .ui_priority(99)
        .semi_hidden()
        .unhide_when_used()
        .render_with(x, overrides);
    Style::numbering("NoList", "No List")
        .default()
        .ui_priority(99)
        .semi_hidden()
        .unhide_when_used()
        .render_with(x, overrides);

    // Heading1..6 — color 2E74B5 (Word's standard accent1 heading
    // blue), Calibri Light via majorHAnsi theme, sizes from the
    // current docx exporter so the visual match holds.
    const HEADING_SIZES_HP: [u32; 6] = [32, 26, 24, 22, 22, 22];
    for level in 1..=6u32 {
        let id = format!("Heading{level}");
        let name = format!("heading {level}");
        let p_pr = PPr {
            keep_next: true,
            keep_lines: true,
            spacing: Some(Spacing {
                before: Some(if level == 1 { 240 } else { 40 }),
                after: Some(0),
                line: None,
            }),
            outline_lvl: Some(level - 1),
            ..Default::default()
        };
        let r_pr = RPr {
            fonts: Some(Fonts {
                ascii_theme: Some("majorHAnsi".into()),
                h_ansi_theme: Some("majorHAnsi".into()),
                east_asia_theme: Some("majorEastAsia".into()),
                cs_theme: Some("majorBidi".into()),
                ..Default::default()
            }),
            color: Some("2E74B5".into()),
            size_hp: Some(HEADING_SIZES_HP[(level - 1) as usize]),
            ..Default::default()
        };
        Style::paragraph(id, name)
            .based_on("Normal")
            .next("Normal")
            .ui_priority(9)
            .q_format()
            .p_pr(p_pr)
            .r_pr(r_pr)
            .render_with(x, overrides);
    }

    // Title — cover page heading. 18pt bold, no theme color.
    Style::paragraph("Title", "Title")
        .based_on("Normal")
        .next("Normal")
        .ui_priority(10)
        .q_format()
        .r_pr(RPr {
            bold: true,
            size_hp: Some(36),
            ..Default::default()
        })
        .render_with(x, overrides);

    // Caption — figure/table captions. Centered by default; authors
    // can override via `style[id:Caption, align:left|right|justify]`.
    Style::paragraph("Caption", "caption")
        .based_on("Normal")
        .next("Normal")
        .ui_priority(35)
        .semi_hidden()
        .unhide_when_used()
        .q_format()
        .p_pr(PPr {
            jc: Some("center".into()),
            ..Default::default()
        })
        .r_pr(RPr {
            italic: true,
            color: Some("44546A".into()),
            size_hp: Some(18),
            ..Default::default()
        })
        .render_with(x, overrides);

    // Hyperlink — blue + underlined character style.
    Style::character("Hyperlink", "Hyperlink")
        .ui_priority(99)
        .r_pr(RPr {
            color: Some("0563C1".into()),
            underline: Some("single".into()),
            ..Default::default()
        })
        .render_with(x, overrides);

    // FootnoteReference — superscript marker character style.
    Style::character("FootnoteReference", "footnote reference")
        .ui_priority(99)
        .semi_hidden()
        .unhide_when_used()
        .r_pr(RPr {
            vert_align: Some("superscript".into()),
            ..Default::default()
        })
        .render_with(x, overrides);

    // TOC1..9 — left/hanging indents widen by 220 per level,
    // matching Word's built-in TOC styles.
    for level in 1..=9u32 {
        let id = format!("TOC{level}");
        let name = format!("toc {level}");
        let p_pr = PPr {
            spacing: Some(Spacing {
                before: Some(0),
                after: Some(100),
                line: None,
            }),
            ind: Some(Indent {
                left: Some(220 * (level - 1)),
                ..Default::default()
            }),
            ..Default::default()
        };
        Style::paragraph(id, name)
            .based_on("Normal")
            .next("Normal")
            .ui_priority(39)
            .semi_hidden()
            .unhide_when_used()
            .p_pr(p_pr)
            .render_with(x, overrides);
    }

    // TOCHeading — the visible "Table of Contents" label sitting
    // immediately above the TOC field. Like Heading1 but without
    // outlineLvl (so the TOC field doesn't include itself).
    Style::paragraph("TOCHeading", "TOC Heading")
        .based_on("Heading1")
        .next("Normal")
        .ui_priority(39)
        .unhide_when_used()
        .q_format()
        .p_pr(PPr {
            keep_next: true,
            keep_lines: true,
            spacing: Some(Spacing {
                before: Some(240),
                after: Some(0),
                line: None,
            }),
            // Centered — matches the reference's "Contents Heading"
            // style. Per-paragraph TOC emission inherits this jc
            // rather than overriding it inline.
            jc: Some("center".into()),
            // No outline_lvl on purpose — keeps TOCHeading out of
            // the TOC field's heading scan.
            ..Default::default()
        })
        .r_pr(RPr {
            fonts: Some(Fonts {
                ascii_theme: Some("majorHAnsi".into()),
                h_ansi_theme: Some("majorHAnsi".into()),
                east_asia_theme: Some("majorEastAsia".into()),
                cs_theme: Some("majorBidi".into()),
                ..Default::default()
            }),
            color: Some("2E74B5".into()),
            size_hp: Some(32),
            ..Default::default()
        })
        .render_with(x, overrides);

    // TableofFigures — caption-style entries in the LoT and LoF.
    Style::paragraph("TableofFigures", "table of figures")
        .based_on("Normal")
        .next("Normal")
        .ui_priority(99)
        .semi_hidden()
        .unhide_when_used()
        .render_with(x, overrides);

    // ListParagraph — used by ordered/unordered lists.
    // contextualSpacing collapses the after-spacing between
    // sibling list items.
    Style::paragraph("ListParagraph", "List Paragraph")
        .based_on("Normal")
        .ui_priority(34)
        .q_format()
        .p_pr(PPr {
            ind: Some(Indent {
                left: Some(720),
                ..Default::default()
            }),
            contextual_spacing: true,
            ..Default::default()
        })
        .render_with(x, overrides);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(s: Style) -> String {
        let mut x = XmlBuf::new();
        s.render(&mut x);
        x.finish()
    }

    #[test]
    fn style_metadata_precedes_pPr_and_rPr() {
        let xml = render(
            Style::paragraph("X", "x")
                .based_on("Normal")
                .ui_priority(10)
                .q_format()
                .p_pr(PPr {
                    keep_next: true,
                    ..Default::default()
                })
                .r_pr(RPr {
                    bold: true,
                    ..Default::default()
                }),
        );
        let name_pos = xml.find("<w:name").unwrap();
        let based_on_pos = xml.find("<w:basedOn").unwrap();
        let priority_pos = xml.find("<w:uiPriority").unwrap();
        let qformat_pos = xml.find("<w:qFormat").unwrap();
        let ppr_pos = xml.find("<w:pPr>").unwrap();
        let rpr_pos = xml.find("<w:rPr>").unwrap();
        assert!(name_pos < based_on_pos);
        assert!(based_on_pos < priority_pos);
        assert!(priority_pos < qformat_pos);
        assert!(qformat_pos < ppr_pos);
        assert!(ppr_pos < rpr_pos);
    }

    #[test]
    fn ppr_keep_next_precedes_spacing_precedes_outlineLvl() {
        let xml = render(
            Style::paragraph("X", "x")
                .based_on("Normal")
                .p_pr(PPr {
                    keep_next: true,
                    keep_lines: true,
                    spacing: Some(Spacing {
                        before: Some(240),
                        after: Some(0),
                        line: None,
                    }),
                    outline_lvl: Some(0),
                    ..Default::default()
                }),
        );
        let kn = xml.find("<w:keepNext/>").unwrap();
        let kl = xml.find("<w:keepLines/>").unwrap();
        let sp = xml.find("<w:spacing").unwrap();
        let ol = xml.find("<w:outlineLvl").unwrap();
        assert!(kn < kl);
        assert!(kl < sp);
        assert!(sp < ol);
    }

    #[test]
    fn rpr_fonts_precedes_bold_color_size_underline() {
        let xml = render(
            Style::character("X", "x").r_pr(RPr {
                fonts: Some(Fonts {
                    ascii: Some("Calibri".into()),
                    ..Default::default()
                }),
                bold: true,
                color: Some("000000".into()),
                size_hp: Some(22),
                underline: Some("single".into()),
                ..Default::default()
            }),
        );
        let f = xml.find("<w:rFonts").unwrap();
        let b = xml.find("<w:b/>").unwrap();
        let c = xml.find("<w:color").unwrap();
        let s = xml.find("<w:sz ").unwrap();
        let u = xml.find("<w:u ").unwrap();
        assert!(f < b);
        assert!(b < c);
        assert!(c < s);
        assert!(s < u);
    }

    #[test]
    fn full_styles_includes_expected_ids() {
        let s = styles();
        for id in [
            "Normal",
            "DefaultParagraphFont",
            "TableNormal",
            "NoList",
            "Heading1",
            "Heading6",
            "Title",
            "Caption",
            "Hyperlink",
            "FootnoteReference",
            "TOC1",
            "TOC9",
            "TOCHeading",
            "TableofFigures",
            "ListParagraph",
        ] {
            assert!(
                s.contains(&format!(r#"w:styleId="{id}""#)),
                "missing styleId {id}"
            );
        }
    }

    #[test]
    fn doc_defaults_come_before_latent_styles_and_real_styles() {
        let s = styles();
        let dd = s.find("<w:docDefaults>").unwrap();
        let ls = s.find("<w:latentStyles ").unwrap();
        let normal = s.find(r#"w:styleId="Normal""#).unwrap();
        assert!(dd < ls);
        assert!(ls < normal);
    }

    #[test]
    fn heading1_has_outlineLvl_0() {
        let s = styles();
        // Find Heading1 and check it contains outlineLvl w:val="0".
        let start = s.find(r#"w:styleId="Heading1""#).unwrap();
        let end = s[start..].find("</w:style>").unwrap() + start;
        let block = &s[start..end];
        assert!(block.contains(r#"<w:outlineLvl w:val="0"/>"#), "block: {block}");
    }
}
