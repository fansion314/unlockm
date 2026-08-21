//! Used Kuwo KWM primitives ported from pinned `umc_kuwo` revision
//! `5dad838c33c3e4aab198d995160d3fbb9968470f`, MIT OR Apache-2.0.

mod kwm_v1;

use crate::crypto::kuwo::kwm_v1::CipherV1;
use crate::crypto::qmc::{QmcError, QmcV2Cipher};
use byteorder::{LE, ReadBytesExt};
use std::fmt;
use std::io::{Cursor, Read};
use thiserror::Error;

#[derive(Debug)]
pub(crate) struct HeaderMagicBytes([u8; 16]);

#[derive(Error, Debug)]
pub(crate) enum KuwoCryptoError {
    #[error("Invalid KWM header magic bytes: {0}")]
    InvalidHeaderMagic(HeaderMagicBytes),
    #[error("KWMv2: EKey required")]
    V2EKeyRequired,
    #[error("KWM: Unsupported version {0}")]
    UnsupportedVersion(usize),
    #[error("{0}")]
    QmcCipher(#[from] QmcError),
    #[error("Failed to read data: {0}")]
    Io(#[from] std::io::Error),
}

impl fmt::Display for HeaderMagicBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(formatter, "{byte:02x} ")?;
        }
        Ok(())
    }
}

pub(crate) const DATA_START_OFFSET: usize = 0x400;

pub(crate) enum Decipher {
    V1(CipherV1),
    V2(QmcV2Cipher),
}

impl Decipher {
    pub(crate) fn new(
        header: &Header,
        ekey: Option<impl AsRef<[u8]>>,
    ) -> Result<Self, KuwoCryptoError> {
        match header.version {
            1 => Ok(Self::V1(CipherV1::new(header.resource_id))),
            2 => match ekey {
                Some(ekey) => Ok(Self::V2(QmcV2Cipher::new_from_ekey(ekey)?)),
                None => Err(KuwoCryptoError::V2EKeyRequired),
            },
            version => Err(KuwoCryptoError::UnsupportedVersion(version as usize)),
        }
    }

    pub(crate) fn decrypt(&self, data: &mut [u8], offset: usize) {
        match self {
            Self::V1(cipher) => cipher.decrypt(data, offset),
            Self::V2(cipher) => cipher.decrypt(data, offset),
        }
    }
}

pub(crate) struct Header {
    version: u32,
    resource_id: u32,
}

impl Header {
    const MAGIC_1: [u8; 16] = *b"yeelion-kuwo-tme";
    const MAGIC_2: [u8; 16] = *b"yeelion-kuwo\0\0\0\0";

    pub(crate) fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, KuwoCryptoError> {
        let mut cursor = Cursor::new(bytes);
        let mut magic = [0_u8; 0x10];
        cursor.read_exact(&mut magic)?;
        let version = cursor.read_u32::<LE>()?;
        let _unknown_1 = cursor.read_u32::<LE>()?;
        let resource_id = cursor.read_u32::<LE>()?;
        let mut ignored = [0_u8; 0x20];
        cursor.read_exact(&mut ignored)?;
        if magic != Self::MAGIC_1 && magic != Self::MAGIC_2 {
            return Err(KuwoCryptoError::InvalidHeaderMagic(HeaderMagicBytes(magic)));
        }
        Ok(Self {
            version,
            resource_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn parses_kwm_v1_header_and_round_trips_payload() {
        let mut header = Vec::new();
        header.extend_from_slice(b"yeelion-kuwo-tme");
        header.extend_from_slice(&1_u32.to_le_bytes());
        header.extend_from_slice(&0_u32.to_le_bytes());
        header.extend_from_slice(&123_456_u32.to_le_bytes());
        header.extend_from_slice(&[0_u8; 0x20]);
        let header = Header::from_bytes(header).unwrap();
        let cipher = Decipher::new(&header, Option::<&str>::None).unwrap();
        let mut payload = b"fLaC deterministic payload".to_vec();
        let original = payload.clone();
        cipher.decrypt(&mut payload, 7);
        cipher.decrypt(&mut payload, 7);
        assert_eq!(payload, original);
    }

    #[test]
    fn kwm_v2_uses_pinned_pc_ekey_and_qmc_cipher() {
        let encoded = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/qmc_pc_enc_v1.bin.b64"),
        )
        .unwrap();
        let footer_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .unwrap();
        let ekey = crate::crypto::qmc::footer::from_byte_slice(&footer_bytes)
            .unwrap()
            .unwrap()
            .ekey
            .unwrap();
        let header = Header {
            version: 2,
            resource_id: 0,
        };
        let cipher = Decipher::new(&header, Some(ekey)).unwrap();
        let source = (0..16_513)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let mut encrypted = source.clone();
        for (index, chunk) in encrypted.chunks_mut(7).enumerate() {
            cipher.decrypt(chunk, index * 7);
        }
        for (index, chunk) in encrypted.chunks_mut(7).enumerate() {
            cipher.decrypt(chunk, index * 7);
        }
        assert_eq!(encrypted, source);
    }
}
