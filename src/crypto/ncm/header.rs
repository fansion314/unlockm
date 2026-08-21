//! Attribution: `umc_ncm::header` from the pinned upstream revision,
//! MIT OR Apache-2.0.

use super::{NcmError, content_key, metadata};
use crate::crypto::common::read_le_u32;
use crc::{CRC_32_ISO_HDLC, Crc};
use std::{cmp::max, io::Read};

const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct NcmFile {
    audio_key_stream: [u8; 256],
    pub(crate) image1: Option<Vec<u8>>,
    pub(crate) image2: Option<Vec<u8>>,
    pub(crate) audio_data_offset: usize,
    metadata: Vec<u8>,
}

impl NcmFile {
    pub(crate) fn new(header: &[u8]) -> Result<Self, NcmError> {
        if header.len() < 14 {
            return Err(NcmError::HeaderTooSmall(14));
        }
        if !header.starts_with(b"CTENFDAM") {
            return Err(NcmError::NotNcm);
        }
        let content_key_length = read_le_u32(&header[10..14]) as usize;
        let mut offset = 14;
        if header.len() < offset + content_key_length + 4 {
            return Err(NcmError::HeaderTooSmall(offset + content_key_length + 4));
        }
        let content_key = &header[offset..offset + content_key_length];
        offset += content_key_length;
        let metadata_length = read_le_u32(&header[offset..offset + 4]) as usize;
        offset += 4;
        if header.len() < offset + metadata_length + 9 {
            return Err(NcmError::HeaderTooSmall(offset + metadata_length + 9));
        }
        let metadata_bytes = &header[offset..offset + metadata_length];
        offset += metadata_length;
        let expected = read_le_u32(&header[offset..offset + 4]);
        let actual = CRC32.checksum(&header[..offset]);
        if actual != expected {
            return Err(NcmError::ChecksumInvalid { expected, actual });
        }
        offset += 4;
        if header[offset] != 1 {
            return Err(NcmError::UnsupportedCoverVersion(header[offset]));
        }
        offset += 1;
        let frame_length = read_le_u32(&header[offset..offset + 4]) as usize;
        offset += 4;
        if header.len() < offset + frame_length + 4 {
            return Err(NcmError::HeaderTooSmall(offset + frame_length + 4));
        }
        let image1_length = read_le_u32(&header[offset..offset + 4]) as usize;
        if image1_length > frame_length {
            return Err(NcmError::InvalidCoverSizes {
                frame_size: frame_length,
                image1_size: image1_length,
            });
        }
        offset += 4;
        let image2_length = frame_length - image1_length;
        let image1 = (image1_length != 0).then(|| header[offset..offset + image1_length].to_vec());
        offset += image1_length;
        let image2 = (image2_length != 0).then(|| header[offset..offset + image2_length].to_vec());
        offset += image2_length;
        Ok(Self {
            audio_key_stream: build_audio_key_stream(content_key)?,
            image1,
            image2,
            audio_data_offset: offset,
            metadata: metadata_bytes.to_vec(),
        })
    }

    pub(crate) fn new_from_readable(reader: &mut impl Read) -> Result<Self, NcmError> {
        let mut bytes_read = 0;
        let mut next_read = 4096;
        let mut header = vec![0; next_read];
        loop {
            if header.len() < bytes_read + next_read {
                header.resize(bytes_read + next_read, 0);
            }
            let count = reader
                .read(&mut header[bytes_read..])
                .map_err(NcmError::Io)?;
            if count == 0 {
                return Err(NcmError::ReadZero);
            }
            bytes_read += count;
            match Self::new(&header) {
                Ok(ncm) => return Ok(ncm),
                Err(NcmError::HeaderTooSmall(expected)) => {
                    next_read = max(expected - bytes_read, 4096)
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn metadata_json(&self) -> Result<Vec<u8>, NcmError> {
        metadata::decrypt(&self.metadata)
    }

    pub(crate) fn decrypt(&self, bytes: &mut [u8], offset: usize) {
        for (byte, offset) in bytes.iter_mut().zip(offset..) {
            *byte ^= self.audio_key_stream[offset % self.audio_key_stream.len()];
        }
    }
}

fn build_audio_key_stream(encrypted_content_key: &[u8]) -> Result<[u8; 256], NcmError> {
    let key = content_key::decrypt(encrypted_content_key)?;
    let mut state = [0_u8; 256];
    for (index, byte) in state.iter_mut().enumerate() {
        *byte = index as u8;
    }
    let mut index = 0_u8;
    for (position, key_byte) in (0..256).zip(key.iter().cycle()) {
        index = index.wrapping_add(state[position]).wrapping_add(*key_byte);
        state.swap(position, index as usize);
    }
    let mut stream = [0_u8; 256];
    for (position, byte) in stream.iter_mut().enumerate() {
        let state_index = (position + 1) & 0xff;
        let value = state[state_index].wrapping_add(state_index as u8);
        *byte = state[state[state_index].wrapping_add(state[value as usize]) as usize];
    }
    Ok(stream)
}
