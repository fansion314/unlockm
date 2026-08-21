//! Attribution: `umc_ncm::metadata` from the pinned upstream revision,
//! MIT OR Apache-2.0.

use super::NcmError;
use aes::{
    cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyInit},
    Aes128Dec,
};
use base64::Engine;

const METADATA_KEY: [u8; 16] = *b"#14ljk_!\\]&0U<'(";

pub(super) fn decrypt(encrypted: &[u8]) -> Result<Vec<u8>, NcmError> {
    let bytes: Vec<u8> = encrypted.iter().map(|byte| byte ^ 0x63).collect();
    let encoded = bytes
        .strip_prefix(b"163 key(Don't modify):")
        .ok_or(NcmError::MetadataPrefix)?;
    let mut bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| NcmError::MetadataBase64)?;
    let cipher = Aes128Dec::new_from_slice(&METADATA_KEY).expect("fixed AES-128 key");
    let plaintext = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut bytes)
        .map_err(|_| NcmError::MetadataDecrypt)?;
    plaintext
        .strip_prefix(b"music:")
        .map(Vec::from)
        .ok_or(NcmError::MetadataJsonPrefix)
}
