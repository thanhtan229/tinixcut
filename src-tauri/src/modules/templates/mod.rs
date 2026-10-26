//! Layered Templates, Parameters & Portable Packs (Phase 32)
//!
//! Provides the authoritative template catalog, curated native definitions,
//! and secure .tinixtpl pack import/export.

pub mod agent;
pub mod capcut_import;
pub mod catalog;
pub mod curated;
pub mod pack;

pub use agent::*;
pub use capcut_import::*;
pub use catalog::*;
pub use curated::*;
pub use pack::*;
