//! Per-element vocabulary definitions.
//!
//! Each submodule defines one element (or one closely related group)
//! as a top-level [`ElementDef`] constant. [`ALL`] collects them for
//! schema lookup and validation dispatch.
//!
//! Convention: file name matches the element name. When the same name
//! has two distinct schemas (e.g. `col` in document vs sheet doc types),
//! both `ElementDef` constants live in one file and both appear in
//! `ALL`; `Registry::get(name, doc_type)` picks the right one.

use crate::element::ElementDef;

// --- Universal inline ---
pub mod code;
pub mod date;
pub mod footnote;
pub mod link;
pub mod math;
pub mod mention;
pub mod text;

// --- Spreadsheet embed ---
pub mod formula;

// --- Document structural ---
pub mod col;
pub mod hr;
pub mod layout;
pub mod pagebreak;
pub mod section;

// --- Headings ---
pub mod heading;

// --- Document block content ---
pub mod blockquote;
pub mod image;
pub mod note;
pub mod paragraph;

// --- Lists ---
pub mod li;
pub mod ol;
pub mod ul;

// --- Tables (document) ---
pub mod cell;
pub mod row;
pub mod table;

// --- Presentation ---
pub mod bullets;
pub mod item;
pub mod slide;
pub mod speaker_note;
pub mod title;
pub mod transition;

// --- Sheet ---
pub mod chart;
pub mod fill;
pub mod format;
pub mod named;
pub mod sheet;
pub mod source;

/// All elements defined in the per-element layout. Schema lookup and
/// validation iterate this slice. Order is not significant for lookup
/// correctness (the registry handles per-doc-type disambiguation).
pub const ALL: &[&ElementDef] = &[
    // Universal inline
    &code::CODE,
    &date::DATE,
    &footnote::FOOTNOTE,
    &link::LINK,
    &math::MATH,
    &mention::MENTION,
    &text::TEXT,
    // Spreadsheet embed
    &formula::FORMULA,
    // Document structural
    &col::COL_LAYOUT,
    &col::COL_SHEET,
    &hr::HR,
    &layout::LAYOUT,
    &pagebreak::PAGEBREAK,
    &section::SECTION,
    // Headings
    &heading::H1,
    &heading::H2,
    &heading::H3,
    &heading::H4,
    &heading::H5,
    &heading::H6,
    // Document block content
    &blockquote::BLOCKQUOTE,
    &image::IMAGE,
    &note::NOTE,
    &paragraph::P,
    // Lists
    &li::LI,
    &ol::OL,
    &ul::UL,
    // Tables (document) + sheet variants
    &cell::CELL_DOC,
    &cell::CELL_SHEET,
    &row::ROW_DOC,
    &row::ROW_SHEET,
    &table::TABLE,
    // Presentation
    &bullets::BULLETS,
    &item::ITEM,
    &slide::SLIDE,
    &speaker_note::SPEAKER_NOTE,
    &title::TITLE,
    &transition::TRANSITION,
    // Sheet
    &chart::CHART,
    &fill::FILL,
    &format::FORMAT,
    &named::NAMED,
    &sheet::SHEET,
    &source::SOURCE,
];
