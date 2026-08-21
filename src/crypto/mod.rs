//! Local, crate-private crypto and audio primitives.
//!
//! Substantial algorithms in this directory are selectively ported from
//! `lib_um_crypto_rust` revision `5dad838c33c3e4aab198d995160d3fbb9968470f`
//! under its MIT OR Apache-2.0 terms. See `THIRD_PARTY_NOTICES.md`.

pub(crate) mod audio;
pub(crate) mod common;
pub(crate) mod kgm;
pub(crate) mod kuwo;
pub(crate) mod ncm;
pub(crate) mod qmc;
pub(crate) mod xiami;
pub(crate) mod xmly;
