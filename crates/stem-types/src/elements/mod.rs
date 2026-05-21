//! Per-element vocabulary definitions.
//!
//! Each submodule defines one element (or one closely related group)
//! as a top-level [`ElementDef`] constant. [`ALL`] collects them for
//! schema lookup and validation dispatch.
//!
//! This is the migration target away from the legacy `BUILTINS` array
//! in [`crate::schema`]. As elements move here, they're removed from
//! `BUILTINS`. Lookups consult `ALL` first, then fall back to the legacy
//! registry — so the migration can proceed one element at a time
//! without breaking existing tests.

use crate::element::ElementDef;

pub mod formula;
pub mod link;

/// All elements defined in the per-element layout. Schema lookup and
/// validation iterate this slice.
pub const ALL: &[&ElementDef] = &[&formula::FORMULA, &link::LINK];
