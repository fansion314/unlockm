use crate::crypto::kgm::KugouError;
use crate::crypto::kgm::header::Header;

pub fn get_slot_key(header: &Header) -> Result<&'static [u8], KugouError> {
    match header.key_slot {
        1 => Ok(b"l,/'"),
        slot => Err(KugouError::UnsupportedKeySlot(slot)),
    }
}
