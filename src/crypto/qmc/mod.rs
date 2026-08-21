//! QMC primitives ported from `umc_qmc` at pinned revision
//! `5dad838c33c3e4aab198d995160d3fbb9968470f` (MIT OR Apache-2.0).
//!
//! The initial local surface exposes v1 while the same task ports v2/footer
//! and ekey before the external dependency is removed.

pub(crate) mod ekey;
pub(crate) mod footer;
pub(crate) mod v1;
pub(crate) mod v2_map;
pub(crate) mod v2_rc4;

#[derive(Debug, thiserror::Error)]
pub(crate) enum QmcError {
    #[error("QMC v2 map key is empty")]
    EmptyMapKey,
    #[error("EKey: {0}")]
    EKey(#[from] ekey::EKeyDecryptError),
}

#[derive(Debug, Clone)]
pub(crate) enum QmcV2Cipher {
    Map(v2_map::Qmc2Map),
    Rc4(v2_rc4::Qmc2Rc4),
}

impl QmcV2Cipher {
    pub(crate) fn new(key: impl AsRef<[u8]>) -> Result<Self, QmcError> {
        let key = key.as_ref();
        match key.len() {
            0 => Err(QmcError::EmptyMapKey),
            1..=300 => Ok(Self::Map(v2_map::Qmc2Map::new(key)?)),
            _ => Ok(Self::Rc4(v2_rc4::Qmc2Rc4::new(key))),
        }
    }

    pub(crate) fn new_from_ekey(ekey: impl AsRef<[u8]>) -> Result<Self, QmcError> {
        Self::new(ekey::decrypt(ekey)?)
    }

    pub(crate) fn decrypt(&self, data: &mut [u8], offset: usize) {
        match self {
            Self::Map(cipher) => cipher.decrypt(data, offset),
            Self::Rc4(cipher) => cipher.decrypt(data, offset),
        }
    }
}
