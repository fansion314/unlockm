//! NetEase NCM header, metadata, and stream cipher ported from `umc_ncm` at
//! `lib_um_crypto_rust` revision `5dad838c33c3e4aab198d995160d3fbb9968470f`.
//!
//! Attribution: MIT OR Apache-2.0. This is intentionally crate-private and
//! preserves the pinned parser/decrypt behavior used by format and metadata
//! adapters.

mod content_key;
pub(crate) mod header;
mod metadata;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum NcmError {
    #[error("header needs at least {0} bytes")]
    HeaderTooSmall(usize),
    #[error("file I/O error: {0}")]
    Io(#[source] std::io::Error),
    #[error("file I/O error: read zero bytes")]
    ReadZero,
    #[error("not an NCM file")]
    NotNcm,
    #[error("invalid NCM checksum: expected {expected:08x}, actual {actual:08x}")]
    ChecksumInvalid { expected: u32, actual: u32 },
    #[error("unsupported cover image version: {0}")]
    UnsupportedCoverVersion(u8),
    #[error("cover frame size {frame_size} is smaller than image one size {image1_size}")]
    InvalidCoverSizes {
        frame_size: usize,
        image1_size: usize,
    },
    #[error("invalid encrypted content key")]
    ContentKeyDecrypt,
    #[error("invalid content key prefix")]
    ContentKeyPrefix,
    #[error("invalid encrypted metadata prefix")]
    MetadataPrefix,
    #[error("invalid metadata base64")]
    MetadataBase64,
    #[error("invalid encrypted metadata")]
    MetadataDecrypt,
    #[error("invalid metadata JSON prefix")]
    MetadataJsonPrefix,
}
