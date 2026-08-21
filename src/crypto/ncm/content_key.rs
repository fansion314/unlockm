//! Attribution: `umc_ncm::content_key` from the pinned upstream revision,
//! MIT OR Apache-2.0.

use super::NcmError;
use aes::{
    cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyInit},
    Aes128Dec,
};

const CONTENT_KEY: [u8; 16] = *b"hzHRAmso5kInbaxW";

pub(super) fn decrypt(encrypted: &[u8]) -> Result<Vec<u8>, NcmError> {
    let mut bytes: Vec<u8> = encrypted.iter().map(|byte| byte ^ 0x64).collect();
    let cipher = Aes128Dec::new_from_slice(&CONTENT_KEY).expect("fixed AES-128 key");
    let plaintext = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut bytes)
        .map_err(|_| NcmError::ContentKeyDecrypt)?;
    plaintext
        .strip_prefix(b"neteasecloudmusic")
        .map(Vec::from)
        .ok_or(NcmError::ContentKeyPrefix)
}
