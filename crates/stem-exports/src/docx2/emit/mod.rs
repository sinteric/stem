//! Body emitters — convert cooked AST blocks into OOXML fragments
//! appended to an [`XmlBuf`].
//!
//! Split by concern so each subsequent migration task can land
//! its slice without churn:
//! - `paragraph`: block-level paragraphs (p, h1..6, title,
//!   blockquote, pagebreak, ...). Active in task 6.
//! - `run`: text-piece → `<w:r><w:t>` runs with run properties.
//!   Plain-text only in task 6; rich formatting arrives in task 7.
//!
//! Later subtasks (7+) add: table, drawing, hyperlink, bookmark,
//! field. Each is a sibling module so the body emission stays
//! linear and grep-friendly.

pub mod ctx;
pub mod drawing;
pub mod field;
pub mod hyperlink;
pub mod paragraph;
pub mod run;
pub mod table;
