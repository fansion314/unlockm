//! Attribution: `umc_qmc::v2_map::key`, pinned upstream revision, MIT OR Apache-2.0.

use super::super::QmcError;

pub(super) fn compress(long_key: &[u8]) -> Result<[u8; 128], QmcError> {
    if long_key.is_empty() {
        return Err(QmcError::EmptyMapKey);
    }
    let mut result = [0; 128];
    for (out, index) in result.iter_mut().zip(0..128) {
        let index = (index * index + 71_214) % long_key.len();
        let value = long_key[index];
        let shift = ((index + 4) % 8) as u32;
        *out = value.wrapping_shl(shift) | value.wrapping_shr(shift);
    }
    Ok(result)
}
