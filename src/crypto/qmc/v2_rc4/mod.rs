//! Attribution: `umc_qmc::v2_rc4`, pinned upstream revision, MIT OR Apache-2.0.

mod cipher;
mod hash;
mod rc4;
mod segment_key;

pub(crate) use cipher::Qmc2Rc4;
