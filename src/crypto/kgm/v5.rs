use crate::crypto::kgm::KugouError;
use crate::crypto::qmc::QmcV2Cipher;

pub struct DecipherV5(QmcV2Cipher);

impl DecipherV5 {
    pub fn new(ekey: &str) -> Result<Self, KugouError> {
        let cipher = QmcV2Cipher::new_from_ekey(ekey)
            .map_err(|e| KugouError::QMC2EKeyError(e.to_string()))?;

        Ok(Self(cipher))
    }

    pub fn decrypt<T: AsMut<[u8]> + ?Sized>(&self, buffer: &mut T, offset: usize) {
        self.0.decrypt(buffer.as_mut(), offset)
    }
}
