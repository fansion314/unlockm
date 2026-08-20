//! `unlockm` application library.  The binary is deliberately thin so batch
//! and watch mode share the exact same planning and processing pipeline.
pub mod app;
pub mod cli;
pub mod config;
pub mod error;
pub mod formats;
pub mod io;
pub mod keys;
pub mod metadata;
pub mod pathing;

pub const PARTIAL_FAILURE_EXIT: i32 = 3;
