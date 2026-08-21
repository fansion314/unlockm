mod key_derive;

use crate::crypto::kgm::KugouError;
use byteorder::{ByteOrder, LE};
use key_derive::decrypt_db_page;

const PAGE_SIZE: usize = 0x400;
const SQLITE_HEADER: [u8; 0x10] = *b"SQLite format 3\0";

fn validate_page_1_header(header: &[u8]) -> Result<(), KugouError> {
    let o10 = LE::read_u32(&header[0x10..0x14]);
    let o14 = LE::read_u32(&header[0x14..0x18]);

    let v6 = ((o10 & 0xff) << 8) | ((o10 & 0xff00) << 16);
    let ok = o14 == 0x20204000 && (v6 - 0x200) <= 0xFE00 && ((v6 - 1) & v6) == 0;
    if !ok {
        Err(KugouError::InvalidPage1Header)?;
    }
    Ok(())
}

pub fn decrypt_db<T: AsMut<[u8]> + ?Sized>(buffer: &mut T) -> Result<(), KugouError> {
    let buffer = buffer.as_mut();
    let db_size = buffer.len();

    // not encrypted
    if buffer.starts_with(&SQLITE_HEADER) {
        return Ok(());
    }

    if db_size % PAGE_SIZE != 0 || db_size == 0 {
        Err(KugouError::InvalidDatabaseSize(db_size))?;
    }

    let last_page = db_size / PAGE_SIZE;

    // page 1 is the header
    decrypt_page_1(&mut buffer[0..PAGE_SIZE])?;

    let mut offset = PAGE_SIZE;
    for page_no in 2..=last_page {
        decrypt_db_page(&mut buffer[offset..offset + PAGE_SIZE], page_no as u32)?;
        offset += PAGE_SIZE;
    }

    Ok(())
}

fn decrypt_page_1(page: &mut [u8]) -> Result<(), KugouError> {
    validate_page_1_header(page)?;

    // Backup expected hdr value
    let mut expected_hdr_value = [0u8; 0x08];
    expected_hdr_value.copy_from_slice(&page[0x10..0x18]);

    // Copy encrypted hdr over
    let (hdr, encrypted_page_data) = page.split_at_mut(0x10);
    encrypted_page_data[0..0x08].copy_from_slice(&hdr[0x08..0x10]);

    decrypt_db_page(encrypted_page_data, 1)?;

    // Validate header
    if encrypted_page_data[..8] != expected_hdr_value[..8] {
        Err(KugouError::DecryptPage1Failed)?;
    }

    // Apply SQLite header
    hdr.copy_from_slice(&SQLITE_HEADER);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_page_one_restores_sqlite_header_and_payload() {
        let expected_header = [0x04, 0x00, 0x01, 0x01, 0x00, 0x40, 0x20, 0x20];
        let mut plain = (0..PAGE_SIZE - 0x10)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        plain[..8].copy_from_slice(&expected_header);
        let mut encrypted = plain.clone();
        key_derive::encrypt_db_page(&mut encrypted, 1);

        let mut database = vec![0_u8; PAGE_SIZE];
        database[0x08..0x10].copy_from_slice(&encrypted[..8]);
        database[0x10..0x18].copy_from_slice(&expected_header);
        database[0x18..].copy_from_slice(&encrypted[8..]);
        decrypt_db(&mut database).unwrap();

        assert_eq!(&database[..0x10], &SQLITE_HEADER);
        assert_eq!(&database[0x10..], plain.as_slice());
    }
}
