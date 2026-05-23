//! `docProps/core.xml` and `docProps/app.xml` — document metadata.
//!
//! These are mostly cosmetic; Word reads them for the file
//! information dialog and Explorer file properties. We emit minimal
//! valid versions with empty title/subject/creator and the current
//! ISO 8601 timestamp for created/modified. Date generation lives
//! here rather than in core.xml itself so tests can override it.

use super::super::xml::XmlBuf;

const NS_CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
const NS_DCTERMS: &str = "http://purl.org/dc/terms/";
const NS_DCMITYPE: &str = "http://purl.org/dc/dcmitype/";
const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const NS_EXT_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";

/// Build `docProps/core.xml` with the given ISO 8601 timestamp.
///
/// All fields default to empty so the file is unauthored. Callers
/// that care about title/creator can extend this signature later.
pub fn core(iso8601: &str) -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem(
        "cp:coreProperties",
        &[
            ("xmlns:cp", NS_CP),
            ("xmlns:dc", NS_DC),
            ("xmlns:dcterms", NS_DCTERMS),
            ("xmlns:dcmitype", NS_DCMITYPE),
            ("xmlns:xsi", NS_XSI),
        ],
        |x| {
            x.empty("dc:title", &[]);
            x.empty("dc:subject", &[]);
            x.empty("dc:creator", &[]);
            x.empty("cp:keywords", &[]);
            x.empty("dc:description", &[]);
            x.empty("cp:lastModifiedBy", &[]);
            x.elem_text("cp:revision", &[], "1", false);
            x.elem_text(
                "dcterms:created",
                &[("xsi:type", "dcterms:W3CDTF")],
                iso8601,
                false,
            );
            x.elem_text(
                "dcterms:modified",
                &[("xsi:type", "dcterms:W3CDTF")],
                iso8601,
                false,
            );
        },
    );
    x.finish()
}

/// Build `docProps/app.xml` — extended properties. We emit only the
/// fields Word actually displays in its document-info dialog;
/// counts (pages, words, lines) default to zero and Word recomputes
/// them on first open.
pub fn app() -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem("Properties", &[("xmlns", NS_EXT_PROPS)], |x| {
        x.elem_text("Application", &[], "Stem", false);
        x.elem_text("DocSecurity", &[], "0", false);
        x.elem_text("ScaleCrop", &[], "false", false);
        x.elem_text("SharedDoc", &[], "false", false);
        x.elem_text("HyperlinksChanged", &[], "false", false);
        x.elem_text("LinksUpToDate", &[], "false", false);
    });
    x.finish()
}

/// Current UTC time in the OOXML-required W3CDTF subset (no
/// fractional seconds, trailing `Z` zone marker). Pulled out so the
/// tests can pass a fixed value.
pub fn now_w3cdtf() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Stem doesn't depend on chrono; we hand-format from epoch
    // seconds. Algorithm: Howard Hinnant's civil_from_days.
    let (y, m, d, hh, mm, ss) = epoch_to_utc(secs as i64);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn epoch_to_utc(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let seconds_in_day = secs.rem_euclid(86_400) as u32;
    let hh = seconds_in_day / 3600;
    let mm = (seconds_in_day / 60) % 60;
    let ss = seconds_in_day % 60;

    // Howard Hinnant, "days from civil".
    let z = days + 719_468;
    let era = if z >= 0 { z / 146_097 } else { (z - 146_096) / 146_097 };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32, hh, mm, ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_has_required_dcterms() {
        let s = core("2026-05-23T00:00:00Z");
        assert!(s.contains("<cp:coreProperties"));
        assert!(s.contains(r#"xsi:type="dcterms:W3CDTF""#));
        assert!(s.contains("2026-05-23T00:00:00Z"));
    }

    #[test]
    fn app_has_application() {
        let s = app();
        assert!(s.contains("<Application>Stem</Application>"));
    }

    #[test]
    fn epoch_to_utc_known_values() {
        // 1970-01-01 00:00:00 UTC — epoch zero.
        assert_eq!(epoch_to_utc(0), (1970, 1, 1, 0, 0, 0));
        // 2000-02-29 00:00:00 UTC — leap year edge case.
        // Y2K leap rule: 2000 IS a leap year (div 400).
        // Computed via `date -u -d '2000-02-29' +%s` = 951782400.
        assert_eq!(epoch_to_utc(951_782_400), (2000, 2, 29, 0, 0, 0));
        // 2026-05-23 12:00:00 UTC. `date -u -d '2026-05-23 12:00:00'
        // +%s` = 1779537600.
        assert_eq!(epoch_to_utc(1_779_537_600), (2026, 5, 23, 12, 0, 0));
        // 1999-12-31 23:59:59 UTC — second before Y2K.
        assert_eq!(epoch_to_utc(946_684_799), (1999, 12, 31, 23, 59, 59));
    }
}
