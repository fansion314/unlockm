//! Attribution: `umc_ncm::metadata` from the pinned upstream revision,
//! MIT OR Apache-2.0.

use super::NcmError;
use aes::{
    Aes128Dec,
    cipher::{
        Block, BlockCipherDecrypt, KeyInit,
        block_padding::{Padding, Pkcs7},
    },
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
    let (blocks, tail) = Block::<Aes128Dec>::slice_as_chunks_mut(&mut bytes);
    if !tail.is_empty() {
        return Err(NcmError::MetadataDecrypt);
    }
    cipher.decrypt_blocks(blocks);
    let plaintext = Pkcs7::unpad_blocks(blocks).map_err(|_| NcmError::MetadataDecrypt)?;
    plaintext
        .strip_prefix(b"music:")
        .map(Vec::from)
        .ok_or(NcmError::MetadataJsonPrefix)
}
