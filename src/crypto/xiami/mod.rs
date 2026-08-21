//! Xiami header and payload transform ported from pinned `umc_xiami` revision
//! `5dad838c33c3e4aab198d995160d3fbb9968470f`, MIT OR Apache-2.0.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum XiamiError {
    #[error("header too small, require at least {0} bytes")]
    HeaderTooSmall(usize),

    #[error("not a xiami file")]
    NotXiamiFile,
}

pub struct XiamiFile {
    pub copy_len: usize,
    key: u8,
}

impl XiamiFile {
    pub fn from_header(buffer: &[u8]) -> Result<Self, XiamiError> {
        if buffer.len() < 0x10 {
            Err(XiamiError::HeaderTooSmall(0x10))?;
        }

        let (copy_len, key) = match buffer[..0x10] {
            [b'i', b'f', b'm', b't', f1, f2, f3, f4, 0xfe, 0xfe, 0xfe, 0xfe, a, b, c, key] => {
                let copy_len = (a as usize) | ((b as usize) << 8) | ((c as usize) << 16);
                let _format = [f1, f2, f3, f4];
                (copy_len, key.wrapping_sub(1))
            }
            _ => Err(XiamiError::NotXiamiFile)?,
        };

        Ok(Self { copy_len, key })
    }

    pub fn decrypt(&self, buffer: &mut [u8]) {
        for b in buffer.iter_mut() {
            *b = self.key.wrapping_sub(*b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header_and_round_trips_cipher() {
        let header = [
            b'i', b'f', b'm', b't', b'f', b'L', b'a', b'C', 0xfe, 0xfe, 0xfe, 0xfe, 0x34, 0x12,
            0x00, 0x80,
        ];
        let file = XiamiFile::from_header(&header).unwrap();
        assert_eq!(file.copy_len, 0x1234);
        let mut bytes = b"payload".to_vec();
        let original = bytes.clone();
        file.decrypt(&mut bytes);
        file.decrypt(&mut bytes);
        assert_eq!(bytes, original);
    }
}
