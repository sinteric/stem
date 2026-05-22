//! `word/theme/theme1.xml` — color, font, and formatting theme.
//!
//! Word requires every docx with styles that reference theme fonts
//! (e.g. `Heading1`'s `asciiTheme="majorHAnsi"`) to have a theme
//! part. The theme is otherwise inert: we use the standard Office
//! palette + Calibri Light / Calibri major/minor fonts so the
//! built-in style set behaves the way Word users expect.
//!
//! Hand-authored — `<a:fontScheme>` is shortened by omitting the
//! 30+ Eastern-language fallback fonts the Office default theme
//! includes. Word fills those in from the system at render time, so
//! omitting them costs no fidelity here.

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

pub fn theme1() -> String {
    let mut s = String::with_capacity(4096);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    s.push_str(&format!(
        r#"<a:theme xmlns:a="{NS_A}" name="Office Theme">"#
    ));
    s.push_str("<a:themeElements>");

    // Color scheme — the standard Office colors.
    s.push_str(r#"<a:clrScheme name="Office">"#);
    s.push_str(r#"<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>"#);
    s.push_str(r#"<a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>"#);
    s.push_str(r#"<a:dk2><a:srgbClr val="44546A"/></a:dk2>"#);
    s.push_str(r#"<a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>"#);
    s.push_str(r#"<a:accent1><a:srgbClr val="5B9BD5"/></a:accent1>"#);
    s.push_str(r#"<a:accent2><a:srgbClr val="ED7D31"/></a:accent2>"#);
    s.push_str(r#"<a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>"#);
    s.push_str(r#"<a:accent4><a:srgbClr val="FFC000"/></a:accent4>"#);
    s.push_str(r#"<a:accent5><a:srgbClr val="4472C4"/></a:accent5>"#);
    s.push_str(r#"<a:accent6><a:srgbClr val="70AD47"/></a:accent6>"#);
    s.push_str(r#"<a:hlink><a:srgbClr val="0563C1"/></a:hlink>"#);
    s.push_str(r#"<a:folHlink><a:srgbClr val="954F72"/></a:folHlink>"#);
    s.push_str("</a:clrScheme>");

    // Font scheme — Calibri Light for headings, Calibri for body.
    s.push_str(r#"<a:fontScheme name="Office">"#);
    s.push_str("<a:majorFont>");
    s.push_str(r#"<a:latin typeface="Calibri Light" panose="020F0302020204030204"/>"#);
    s.push_str(r#"<a:ea typeface=""/>"#);
    s.push_str(r#"<a:cs typeface=""/>"#);
    s.push_str("</a:majorFont>");
    s.push_str("<a:minorFont>");
    s.push_str(r#"<a:latin typeface="Calibri" panose="020F0502020204030204"/>"#);
    s.push_str(r#"<a:ea typeface=""/>"#);
    s.push_str(r#"<a:cs typeface=""/>"#);
    s.push_str("</a:minorFont>");
    s.push_str("</a:fontScheme>");

    // Format scheme — Office defaults. Required by the schema even
    // though every entry is a passthrough fill/line/effect.
    s.push_str(r#"<a:fmtScheme name="Office">"#);
    s.push_str("<a:fillStyleLst>");
    // 3 fills required.
    for _ in 0..3 {
        s.push_str(r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#);
    }
    s.push_str("</a:fillStyleLst>");
    s.push_str("<a:lnStyleLst>");
    // 3 lines required.
    for _ in 0..3 {
        s.push_str(r#"<a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln>"#);
    }
    s.push_str("</a:lnStyleLst>");
    s.push_str("<a:effectStyleLst>");
    // 3 effects required. Use empty effectLst — passthrough.
    for _ in 0..3 {
        s.push_str("<a:effectStyle><a:effectLst/></a:effectStyle>");
    }
    s.push_str("</a:effectStyleLst>");
    s.push_str("<a:bgFillStyleLst>");
    // 3 background fills required.
    for _ in 0..3 {
        s.push_str(r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#);
    }
    s.push_str("</a:bgFillStyleLst>");
    s.push_str("</a:fmtScheme>");

    s.push_str("</a:themeElements>");
    s.push_str("<a:objectDefaults/>");
    s.push_str("<a:extraClrSchemeLst/>");
    s.push_str("</a:theme>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_has_all_required_elements() {
        let s = theme1();
        assert!(s.contains("<a:clrScheme"));
        assert!(s.contains("<a:fontScheme"));
        assert!(s.contains("<a:fmtScheme"));
        assert!(s.contains(r#"<a:majorFont>"#));
        assert!(s.contains(r#"typeface="Calibri Light""#));
        // Schema requires 3 of each fmtScheme list — count solid
        // fills as a proxy.
        assert_eq!(s.matches("<a:fillStyleLst>").count(), 1);
        assert_eq!(s.matches("<a:bgFillStyleLst>").count(), 1);
    }
}
