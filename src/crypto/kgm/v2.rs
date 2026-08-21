use crate::crypto::kgm::KugouError;
use crate::crypto::kgm::header::Header;
use crate::crypto::kgm::slot_keys::get_slot_key;

pub struct DecipherV2 {
    key: [u8; 4],
}

impl DecipherV2 {
    pub fn new(header: &Header) -> Result<Self, KugouError> {
        let mut key = [0u8; 4];
        key.copy_from_slice(get_slot_key(header)?);
        Ok(Self { key })
    }

    pub fn decrypt<T: AsMut<[u8]> + ?Sized>(&self, buffer: &mut T, offset: usize) {
        for (datum, offset) in buffer.as_mut().iter_mut().zip(offset..) {
            *datum ^= self.key[offset % self.key.len()];
        }
    }
}
