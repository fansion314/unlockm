//! Attribution: `umc_qmc::v2_map`, pinned upstream revision, MIT OR Apache-2.0.

mod key;

use super::{QmcError, v1};

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Qmc2Map {
    key: [u8; 128],
}

impl Qmc2Map {
    pub(crate) fn new(key: &[u8]) -> Result<Self, QmcError> {
        Ok(Self {
            key: key::compress(key)?,
        })
    }

    pub(crate) fn decrypt(&self, data: &mut [u8], offset: usize) {
        for (index, byte) in data.iter_mut().enumerate() {
            let mut one = [*byte];
            v1::transform_with_key(&mut one, offset + index, &self.key);
            *byte = one[0];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_map_vector() {
        let key = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .cycle()
            .take(325)
            .collect::<Vec<_>>();
        let cipher = Qmc2Map::new(&key).unwrap();
        let mut data = [
            0x00u8, 0x9e, 0x41, 0xc1, 0x71, 0x36, 0x00, 0x80, 0xf4, 0x00, 0x75, 0x9e, 0x36, 0x00,
            0x14, 0x8a,
        ];
        cipher.decrypt(&mut data, 32760);
        assert_eq!(data, [0; 16]);
    }
}
