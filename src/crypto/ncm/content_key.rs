//! Attribution: `umc_ncm::content_key` from the pinned upstream revision,
//! MIT OR Apache-2.0.

use super::NcmError;
use aes::{
    Aes128Dec,
    cipher::{
        Block, BlockCipherDecrypt, KeyInit,
        block_padding::{Padding, Pkcs7},
    },
};

const CONTENT_KEY: [u8; 16] = *b"hzHRAmso5kInbaxW";

pub(super) fn decrypt(encrypted: &[u8]) -> Result<Vec<u8>, NcmError> {
    let mut bytes: Vec<u8> = encrypted.iter().map(|byte| byte ^ 0x64).collect();
    let cipher = Aes128Dec::new_from_slice(&CONTENT_KEY).expect("fixed AES-128 key");
    let (blocks, tail) = Block::<Aes128Dec>::slice_as_chunks_mut(&mut bytes);
    if !tail.is_empty() {
        return Err(NcmError::ContentKeyDecrypt);
    }
    cipher.decrypt_blocks(blocks);
    let plaintext = Pkcs7::unpad_blocks(blocks).map_err(|_| NcmError::ContentKeyDecrypt)?;
    plaintext
        .strip_prefix(b"neteasecloudmusic")
        .map(Vec::from)
        .ok_or(NcmError::ContentKeyPrefix)
}
