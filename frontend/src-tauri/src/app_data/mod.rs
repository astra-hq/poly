//! Poly app data paths and legacy-to-Poly migration.
//!
//! The `paths` module provides Poly default paths and legacy candidate
//! enumeration. The `migration` module implements the copy-forward
//! strategy that moves legacy data into Poly's app data directory
//! without deleting anything.

pub mod migration;
pub mod paths;

pub use migration::migrate_if_needed;
pub use paths::{
    legacy_app_data_candidates, legacy_media_dir, legacy_meetily_app_data_dir,
    legacy_meetily_data_dir, poly_media_dir,
};
