//! `word/webSettings.xml` — settings used when Word renders to web
//! preview (Word's "Web Layout" view). For our purposes only three
//! flags matter, all bare markers.

use super::super::xml::XmlBuf;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

pub fn web_settings() -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem("w:webSettings", &[("xmlns:w", NS_W)], |x| {
        x.empty("w:optimizeForBrowser", &[]);
        x.empty("w:relyOnVML", &[]);
        x.empty("w:allowPNG", &[]);
    });
    x.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_settings_has_three_flags() {
        let s = web_settings();
        assert!(s.contains("<w:optimizeForBrowser/>"));
        assert!(s.contains("<w:relyOnVML/>"));
        assert!(s.contains("<w:allowPNG/>"));
    }
}
