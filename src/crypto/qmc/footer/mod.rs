//! QMC Android and PC footer parsers ported from pinned `umc_qmc` revision
//! `5dad838c33c3e4aab198d995160d3fbb9968470f`, MIT OR Apache-2.0.

pub mod android_qtag;
pub mod android_stag;
mod musicex_v1;
pub mod pc_v1_legacy;
pub mod pc_v2_musicex;
mod utils;

use crate::crypto::qmc::footer::{
    android_qtag::QTagMetadata, android_stag::STagMetadata, pc_v1_legacy::PcV1Legacy,
    pc_v2_musicex::PcV2MusicEx,
};
use thiserror::Error;

pub const INITIAL_DETECTION_LEN: usize = 1024;

#[derive(Error, Debug)]
pub enum FooterParseError {
    #[error("Footer: Buffer too small, require at least {0} bytes")]
    BufferTooSmall(usize),
    #[error("PCv1/EKey: Buffer too large, might not be valid EKey (len={0})")]
    PCv1EKeyTooLarge(usize),
    #[error("PCv1/EKey: Found invalid EKey char")]
    PCv1EKeyInvalid,

    #[error("PCv2/MusicEx: Invalid metadata version {0}")]
    PCv2InvalidVersion(u32),
    #[error("PCv2/MusicEx: Invalid `MusicEx` size: {0}")]
    PCv2MusicExUnsupportedPayloadSize(usize),

    #[error("Android/STag: Invalid ID field: {0}")]
    STagInvalidId(String),
    #[error("Android/STag: Invalid Version: {0}")]
    STagInvalidVersion(String),
    #[error("Android/STag: Invalid CSV metadata: {0}")]
    STagInvalidCSV(String),

    #[error("Android/QTag: Invalid ID field: {0}")]
    QTagInvalidId(String),
    #[error("Android/QTag: Invalid Version: {0}")]
    QTagInvalidVersion(String),
    #[error("Android/QTag: Invalid EKey field: {0}")]
    QTagInvalidEKey(String),

    #[error("Parse: Failed to parse string '{0}' as integer")]
    StringToIntError(String),

    #[error("Failed to parse MusicExV1: {0}")]
    MusicEx1ParseError(std::io::Error),
}

/// Footer type
#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    /// No extra metadata.
    PCv1Legacy(pc_v1_legacy::PcV1Legacy),
    /// "MusicEx" footer.
    PCv2MusicEx(pc_v2_musicex::PcV2MusicEx),

    /// Android "QTag", with ekey.
    AndroidQTag(android_qtag::QTagMetadata),
    /// Android "STag", metadata only.
    AndroidSTag(android_stag::STagMetadata),
}

/// File Footer metadata
#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
    /// Footer size to trim off.
    pub size: usize,

    /// Embedded key (not decrypted).
    pub ekey: Option<String>,

    /// data/type
    pub data: Data,
}

pub trait MetadataParser {
    fn from_byte_slice(buffer: &[u8]) -> Result<Option<Metadata>, FooterParseError>;
}

pub fn from_byte_slice(buffer: &[u8]) -> Result<Option<Metadata>, FooterParseError> {
    if let Some(metadata) = STagMetadata::from_byte_slice(buffer)? {
        return Ok(Some(metadata));
    }
    if let Some(metadata) = QTagMetadata::from_byte_slice(buffer)? {
        return Ok(Some(metadata));
    }
    if let Some(metadata) = PcV2MusicEx::from_byte_slice(buffer)? {
        return Ok(Some(metadata));
    }
    if let Some(metadata) = PcV1Legacy::from_byte_slice(buffer)? {
        return Ok(Some(metadata));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn tagged(payload: &str, marker: &[u8; 4]) -> Vec<u8> {
        let mut bytes = payload.as_bytes().to_vec();
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(marker);
        bytes
    }

    #[test]
    fn parses_android_qtag_and_stag_vectors() {
        let qtag = from_byte_slice(&tagged("YWJj,123,2", b"QTag"))
            .unwrap()
            .unwrap();
        assert_eq!(qtag.ekey.as_deref(), Some("YWJj"));
        assert_eq!(qtag.size, 18);
        assert_eq!(
            qtag.data,
            Data::AndroidQTag(android_qtag::QTagMetadata { resource_id: 123 })
        );

        let stag = from_byte_slice(&tagged("456,2,media-mid", b"STag"))
            .unwrap()
            .unwrap();
        assert_eq!(stag.ekey, None);
        assert_eq!(stag.size, 23);
        assert_eq!(
            stag.data,
            Data::AndroidSTag(android_stag::STagMetadata {
                resource_id: 456,
                media_mid: "media-mid".into(),
            })
        );
    }

    #[test]
    fn parses_pinned_pc_v2_musicex_vector() {
        let encoded = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/qmc_pc_enc_v2.bin.b64"),
        )
        .unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .unwrap();
        let metadata = from_byte_slice(&bytes).unwrap().unwrap();
        assert_eq!(metadata.ekey, None);
        assert_eq!(metadata.size, 0xc0);
        assert_eq!(
            metadata.data,
            Data::PCv2MusicEx(pc_v2_musicex::PcV2MusicEx {
                mid: "AaBbCcDdEeFfGg".into(),
                media_filename: "F0M000112233445566.mflac".into(),
            })
        );
    }
}
