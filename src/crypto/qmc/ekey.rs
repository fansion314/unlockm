//! Attribution: `umc_qmc::ekey` from pinned revision
//! `5dad838c33c3e4aab198d995160d3fbb9968470f`, MIT OR Apache-2.0.

use crate::crypto::common::base64_decode;
use std::{ops::Mul, sync::OnceLock};
use tc_tea::TcTeaError;
use thiserror::Error;

/// Base64 encoded prefix: `QQMusic EncV2,Key:`.
const EKEY_V2_PREFIX: &[u8; 24] = b"UVFNdXNpYyBFbmNWMixLZXk6";
const EKEY_V2_KEY1: [u8; 16] = [
    0x33, 0x38, 0x36, 0x5a, 0x4a, 0x59, 0x21, 0x40, 0x23, 0x2a, 0x24, 0x25, 0x5e, 0x26, 0x29, 0x28,
];
const EKEY_V2_KEY2: [u8; 16] = [
    0x2a, 0x2a, 0x23, 0x21, 0x28, 0x23, 0x24, 0x25, 0x26, 0x5e, 0x61, 0x31, 0x63, 0x5a, 0x2c, 0x54,
];

static EKEY_SIMPLE_KEY: OnceLock<[u8; 8]> = OnceLock::new();

#[derive(Debug, PartialEq, Error)]
pub(crate) enum EKeyDecryptError {
    #[error("EKey is too short for decryption")]
    EKeyTooShort,
    #[error("Error when decrypting ekey v1: {0}")]
    FailDecryptV1(TcTeaError),
    #[error("Error when decrypting ekey v2: {0}")]
    FailDecryptV2(TcTeaError),
    #[error("Failed to decode b64 content: {0}")]
    Base64Decode(#[from] base64::DecodeError),
}

fn make_simple_key<const N: usize>() -> [u8; N] {
    let mut result = [0_u8; N];
    for (index, value) in result.iter_mut().enumerate() {
        let index = index as f32;
        *value = (106.0 + index * 0.1).tan().abs().mul(100.0) as u8;
    }
    result
}

pub(crate) fn decrypt_v1(ekey: &[u8]) -> Result<Vec<u8>, EKeyDecryptError> {
    if ekey.len() < 12 {
        return Err(EKeyDecryptError::EKeyTooShort);
    }
    let ekey = base64_decode(ekey)?;
    let (header, cipher) = ekey.split_at(8);
    let tea_key = EKEY_SIMPLE_KEY
        .get_or_init(make_simple_key::<8>)
        .iter()
        .zip(header)
        .flat_map(|(&simple, &header)| [simple, header])
        .collect::<Vec<_>>();
    let plaintext = tc_tea::decrypt(cipher, tea_key).map_err(EKeyDecryptError::FailDecryptV1)?;
    Ok([header, &plaintext].concat())
}

pub(crate) fn decrypt_v2(ekey: &[u8]) -> Result<Vec<u8>, EKeyDecryptError> {
    let ekey = base64_decode(ekey)?;
    let ekey = tc_tea::decrypt(ekey, EKEY_V2_KEY1).map_err(EKeyDecryptError::FailDecryptV2)?;
    let ekey = tc_tea::decrypt(ekey, EKEY_V2_KEY2).map_err(EKeyDecryptError::FailDecryptV2)?;
    let ekey = ekey
        .into_iter()
        .take_while(|&byte| byte != 0)
        .collect::<Vec<_>>();
    decrypt_v1(&ekey)
}

pub(crate) fn decrypt(ekey: impl AsRef<[u8]>) -> Result<Vec<u8>, EKeyDecryptError> {
    match ekey.as_ref().strip_prefix(EKEY_V2_PREFIX) {
        Some(v2_ekey) => decrypt_v2(v2_ekey),
        None => decrypt_v1(ekey.as_ref()),
    }
}

#[cfg(test)]
pub(crate) fn encode_v2_for_test(v1_ekey: &[u8]) -> String {
    use base64::Engine;
    let stage2 = tc_tea::encrypt(v1_ekey, EKEY_V2_KEY2).unwrap();
    let stage1 = tc_tea::encrypt(stage2, EKEY_V2_KEY1).unwrap();
    format!(
        "{}{}",
        std::str::from_utf8(EKEY_V2_PREFIX).unwrap(),
        base64::engine::general_purpose::STANDARD.encode(stage1)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_v1_key() {
        assert_eq!(decrypt_v1(b"short"), Err(EKeyDecryptError::EKeyTooShort));
    }
}
