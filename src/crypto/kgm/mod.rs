//! KGM headers, ciphers, and database decrypt ported from pinned `umc_kgm`
//! revision `5dad838c33c3e4aab198d995160d3fbb9968470f`, MIT OR Apache-2.0.

pub mod header;
mod pc_db_decrypt;
mod slot_keys;
pub mod v2;
pub mod v3;
mod v5;

pub use pc_db_decrypt::decrypt_db;

use crate::crypto::kgm::header::Header;
use crate::crypto::kgm::v2::DecipherV2;
use crate::crypto::kgm::v3::DecipherV3;
use thiserror::Error;

use crate::crypto::kgm::v5::DecipherV5;
use block_padding::Error as PaddingError;

#[derive(Debug, Error)]
pub enum KugouError {
    #[error("Header too small, need at least {0} bytes.")]
    HeaderTooSmall(usize),

    #[error("Unsupported key slot: {0}")]
    UnsupportedKeySlot(i32),

    #[error("Unsupported cipher version: {0}")]
    UnsupportedCipherVersion(u32),

    #[error("V5 requires ekey.")]
    V5EKeyRequired,

    #[error("Not KGM File (magic mismatch)")]
    NotKGMFile,

    #[error("Unsupported cipher (self-test failed)")]
    SelfTestFailed,

    #[error("Failed decrypt kugou db data: {0}")]
    DecryptKugouDbError(PaddingError),

    #[error("Invalid database size: {0}")]
    InvalidDatabaseSize(usize),

    #[error("Failed to decrypt page 1 (invalid header)")]
    DecryptPage1Failed,

    #[error("Database does not seem valid")]
    InvalidPage1Header,

    #[error("QMC2EKeyError: {0}")]
    QMC2EKeyError(String),

    #[error("Parse KGM header with i/o error: {0}")]
    HeaderParseIOError(std::io::Error),

    #[error("Invalid audio hash size: {0}")]
    HeaderInvalidAudioHash(usize),
}

pub enum Decipher {
    V2(DecipherV2),
    V3(DecipherV3),
    V5(DecipherV5),
}

impl Decipher {
    pub fn new_v5(header: &Header, ekey: Option<String>) -> Result<Self, KugouError> {
        let decipher = match header.crypto_version {
            2 => Decipher::V2(DecipherV2::new(header)?),
            3 => Decipher::V3(DecipherV3::new(header)?),
            5 => match ekey {
                Some(ekey) => Decipher::V5(DecipherV5::new(&ekey)?),
                _ => Err(KugouError::V5EKeyRequired)?,
            },
            version => Err(KugouError::UnsupportedCipherVersion(version))?,
        };

        let mut test_data = header.decrypt_test_data;
        decipher.decrypt(&mut test_data, 0);
        if test_data != header.get_challenge_data() {
            Err(KugouError::SelfTestFailed)?;
        }

        Ok(decipher)
    }

    pub fn decrypt<T: AsMut<[u8]> + ?Sized>(&self, buffer: &mut T, offset: usize) {
        match self {
            Decipher::V2(decipher) => decipher.decrypt(buffer, offset),
            Decipher::V3(decipher) => decipher.decrypt(buffer, offset),
            Decipher::V5(decipher) => decipher.decrypt(buffer, offset),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn fixture(name: &str) -> Vec<u8> {
        let encoded = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap();
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .unwrap()
    }

    fn assert_header_and_stream(name: &str, version: u32) {
        let header = Header::from_buffer(fixture(name)).unwrap();
        assert_eq!(header.crypto_version, version);
        assert_eq!(header.key_slot, 1);
        // The pinned upstream v2/v3 fixtures are constructor/header vectors;
        // upstream does not run Decipher's challenge self-test on them.
        let cipher = match version {
            2 => Decipher::V2(DecipherV2::new(&header).unwrap()),
            3 => Decipher::V3(DecipherV3::new(&header).unwrap()),
            _ => unreachable!(),
        };

        let source = (0..16_513)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let mut whole = source.clone();
        cipher.decrypt(&mut whole, 0);
        let expected: [u8; 64] = match version {
            2 => [
                0x6c, 0x2d, 0x2d, 0x24, 0x68, 0x29, 0x29, 0x20, 0x64, 0x25, 0x25, 0x2c, 0x60, 0x21,
                0x21, 0x28, 0x7c, 0x3d, 0x3d, 0x34, 0x78, 0x39, 0x39, 0x30, 0x74, 0x35, 0x35, 0x3c,
                0x70, 0x31, 0x31, 0x38, 0x4c, 0x0d, 0x0d, 0x04, 0x48, 0x09, 0x09, 0x00, 0x44, 0x05,
                0x05, 0x0c, 0x40, 0x01, 0x01, 0x08, 0x5c, 0x1d, 0x1d, 0x14, 0x58, 0x19, 0x19, 0x10,
                0x54, 0x15, 0x15, 0x1c, 0x50, 0x11, 0x11, 0x18,
            ],
            3 => [
                0x3c, 0xae, 0x04, 0x9a, 0xcb, 0xc1, 0xf9, 0x8d, 0xa2, 0x63, 0xb9, 0xdb, 0x03, 0xf4,
                0xa2, 0x46, 0xcf, 0xdb, 0x6d, 0xb5, 0x56, 0xed, 0xa5, 0xc7, 0xb9, 0x5c, 0x41, 0xf7,
                0x07, 0x65, 0x58, 0x56, 0x27, 0x28, 0x18, 0xdc, 0x79, 0x70, 0x89, 0x9b, 0xf3, 0x47,
                0x7e, 0x0f, 0x2b, 0x61, 0xc9, 0xac, 0x37, 0xc0, 0xeb, 0xa9, 0x10, 0x5f, 0x14, 0xb7,
                0xaf, 0x0d, 0x65, 0x30, 0xd3, 0x4d, 0xcd, 0x3d,
            ],
            _ => unreachable!(),
        };
        assert_eq!(&whole[..expected.len()], &expected);
        for chunk_size in [1, 7, 16, 4096] {
            let mut chunked = source.clone();
            for (index, chunk) in chunked.chunks_mut(chunk_size).enumerate() {
                cipher.decrypt(chunk, index * chunk_size);
            }
            assert_eq!(chunked, whole, "{name}, chunk={chunk_size}");
        }
    }

    #[test]
    fn pinned_v2_and_v3_headers_initialize_and_match_chunked_transform() {
        assert_header_and_stream("kgm_v2_hdr.bin.b64", 2);
        assert_header_and_stream("kgm_v3_hdr.bin.b64", 3);
    }

    #[test]
    fn pinned_invalid_magic_is_rejected() {
        assert!(matches!(
            Header::from_buffer(fixture("kgm_invalid_magic.bin.b64")),
            Err(KugouError::NotKGMFile)
        ));
    }
}
