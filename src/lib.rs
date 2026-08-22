//! `unlockm` application library.  The binary is deliberately thin so batch
//! and watch mode share the exact same planning and processing pipeline.
pub mod app;
pub mod cli;
pub mod config;
pub(crate) mod crypto;
pub mod error;
pub mod formats;
pub mod io;
pub mod keys;
pub mod metadata;
pub mod pathing;
pub mod presentation;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/support/ncm_fixture.rs"]
pub(crate) mod test_ncm_fixture;

pub const PARTIAL_FAILURE_EXIT: i32 = 3;
