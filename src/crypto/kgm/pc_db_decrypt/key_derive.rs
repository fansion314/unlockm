use crate::crypto::common::md5;
use byteorder::{ByteOrder, LE};

use aes::cipher::{Array, BlockModeDecrypt, KeyIvInit, block_padding::NoPadding};

use crate::crypto::kgm::KugouError;

type Aes128CbcDec = cbc::Decryptor<aes::Aes128Dec>;

const DEFAULT_MASTER_KEY: [u8; 0x18] = [
    // master key (0x10 bytes)
    0x1D, 0x61, 0x31, 0x45, 0xB2, 0x47, 0xBF, 0x7F, 0x3D, 0x18, 0x96, 0x72, 0x14, 0x4F, 0xE4, 0xBF,
    0x00, 0x00, 0x00, 0x00, // page number (le)
    0x73, 0x41, 0x6C, 0x54, // fixed value
];

fn next_page_iv(seed: u32) -> u32 {
    let left = seed.wrapping_mul(0x9EF4);
    let right = seed.wrapping_div(0xce26).wrapping_mul(0x7FFFFF07);
    let value = left.wrapping_sub(right);
    match value & 0x8000_0000 {
        0 => value,
        _ => value.wrapping_add(0x7FFF_FF07),
    }
}

fn derive_page_aes_key(seed: u32) -> [u8; 0x10] {
    let mut master_key = DEFAULT_MASTER_KEY;
    LE::write_u32(&mut master_key[0x10..0x14], seed);
    md5(master_key)
}

fn derive_page_aes_iv(seed: u32) -> [u8; 0x10] {
    let mut buffer = [0u8; 0x10];
    let mut iv = seed + 1;
    for i in (0..0x10).step_by(4) {
        iv = next_page_iv(iv);
        LE::write_u32(&mut buffer[i..i + 4], iv);
    }
    md5(buffer)
}

/// Page number starts from 1.
/// Buffer should have size of ().
pub fn decrypt_db_page(buffer: &mut [u8], page_number: u32) -> Result<(), KugouError> {
    let key = derive_page_aes_key(page_number);
    let iv = derive_page_aes_iv(page_number);

    let key = Array::from(key);
    let iv = Array::from(iv);
    let dec = Aes128CbcDec::new(&key, &iv);
    dec.decrypt_padded::<NoPadding>(buffer)
        .map_err(KugouError::DecryptKugouDbError)?;
    Ok(())
}

#[cfg(test)]
pub(super) fn encrypt_db_page(buffer: &mut [u8], page_number: u32) {
    use aes::cipher::BlockModeEncrypt;
    type Aes128CbcEnc = cbc::Encryptor<aes::Aes128Enc>;
    let key = Array::from(derive_page_aes_key(page_number));
    let iv = Array::from(derive_page_aes_iv(page_number));
    Aes128CbcEnc::new(&key, &iv)
        .encrypt_padded::<NoPadding>(buffer, buffer.len())
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_derive(page: u32, key: [u8; 16], iv: [u8; 16]) {
        assert_eq!(derive_page_aes_key(page), key);
        assert_eq!(derive_page_aes_iv(page), iv);
    }

    #[test]
    fn pinned_page_key_and_iv_vectors() {
        assert_derive(
            0,
            [
                0x19, 0x62, 0xc0, 0x5f, 0xa2, 0xeb, 0xbe, 0x24, 0x28, 0xff, 0x52, 0x2b, 0x9e, 0x03,
                0xea, 0xd4,
            ],
            [
                0x05, 0x5a, 0x67, 0x35, 0x93, 0x89, 0x2d, 0xdf, 0x3a, 0xb3, 0xb3, 0xc6, 0x21, 0xc3,
                0x48, 0x02,
            ],
        );
        assert_derive(
            12_345,
            [
                0xc1, 0x70, 0x06, 0x4e, 0xf8, 0x1e, 0x15, 0x35, 0xc2, 0x9a, 0x65, 0xe4, 0xb6, 0xf5,
                0x78, 0xe9,
            ],
            [
                0xd0, 0xcd, 0x91, 0xd0, 0x23, 0xc5, 0x1e, 0x21, 0xbc, 0x01, 0xaa, 0xd2, 0x81, 0x4c,
                0x9b, 0xb8,
            ],
        );
        assert_derive(
            498_651_347,
            [
                0x5a, 0x69, 0xb3, 0xdc, 0x58, 0xca, 0x16, 0x2e, 0xb4, 0xa7, 0x71, 0x4e, 0xf2, 0x73,
                0x6b, 0xf7,
            ],
            [
                0x62, 0xa7, 0x22, 0x26, 0x64, 0x08, 0x89, 0xb8, 0xff, 0x5d, 0xdc, 0x31, 0x7e, 0x7c,
                0x7e, 0xcc,
            ],
        );
    }

    #[test]
    fn encrypted_page_round_trips() {
        let original = (0..0x400)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let mut encrypted = original.clone();
        encrypt_db_page(&mut encrypted, 7);
        assert_ne!(encrypted, original);
        decrypt_db_page(&mut encrypted, 7).unwrap();
        assert_eq!(encrypted, original);
    }
}
