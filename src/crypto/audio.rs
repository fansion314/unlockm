//! Bounded standard-audio detection ported from `um_audio` at
//! `lib_um_crypto_rust` revision `5dad838c33c3e4aab198d995160d3fbb9968470f`.
//!
//! Attribution: MIT OR Apache-2.0. This local copy intentionally retains only
//! the public type detection used by `unlockm`, not upstream metadata APIs.

use super::common::{read_be_u32, read_le_u32};
use std::fmt::Display;
use thiserror::Error;

const SYNC_FRAME_TEST_SIZE: usize = 4096;
const MAGIC_FLAC: [u8; 4] = *b"fLaC";
const MAGIC_OGG: [u8; 4] = *b"OggS";
const MAGIC_DFF: [u8; 4] = *b"FRM8";
const MAGIC_WMA: [u8; 4] = [0x30, 0x26, 0xB2, 0x75];
const MAGIC_WAV: [u8; 4] = *b"RIFF";
const MAGIC_APE: [u8; 4] = *b"MAC ";
const MAGIC_MKV: [u8; 4] = [0x1a, 0x45, 0xdf, 0xa3];

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub(crate) enum AudioError {
    #[error("require at least {0} bytes of header")]
    NeedMoreHeader(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioType {
    Unknown,
    Ogg,
    Aac,
    Mp3,
    M4a,
    M4b,
    Mp4,
    Wma,
    Mkv,
    Flac,
    Dff,
    Wav,
    Ape,
}

impl AudioType {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Ogg => "ogg",
            Self::Aac => "aac",
            Self::Mp3 => "mp3",
            Self::M4a => "m4a",
            Self::M4b => "m4b",
            Self::Mp4 => "mp4",
            Self::Wma => "wma",
            Self::Flac => "flac",
            Self::Dff => "dff",
            Self::Wav => "wav",
            Self::Ape => "ape",
            Self::Mkv => "mka",
            Self::Unknown => "bin",
        }
    }
}

impl Display for AudioType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.extension())
    }
}

pub(crate) fn detect_audio_type(buffer: &[u8]) -> Result<AudioType, AudioError> {
    let offset = header_metadata_size(buffer, 0)?;
    if buffer.len() < offset + 0x10 {
        return Err(AudioError::NeedMoreHeader(offset + 0x10));
    }
    let buffer = &buffer[offset..];
    let magic: [u8; 4] = buffer[..4].try_into().expect("minimum header checked");
    match magic {
        MAGIC_FLAC => return Ok(AudioType::Flac),
        MAGIC_OGG => return Ok(AudioType::Ogg),
        MAGIC_DFF => return Ok(AudioType::Dff),
        MAGIC_WMA => return Ok(AudioType::Wma),
        MAGIC_WAV => return Ok(AudioType::Wav),
        MAGIC_APE => return Ok(AudioType::Ape),
        MAGIC_MKV => return Ok(AudioType::Mkv),
        _ => {}
    }
    if is_aac(read_be_u32(&magic)) {
        return Ok(AudioType::Aac);
    }
    if is_mp3(buffer) {
        return Ok(AudioType::Mp3);
    }
    if &buffer[4..8] == b"ftyp" {
        match &buffer[8..12] {
            b"isom" | b"iso2" | b"MSNV" => return Ok(AudioType::Mp4),
            b"NDAS" => return Ok(AudioType::M4a),
            _ => {}
        }
        match &buffer[8..11] {
            b"M4A" => return Ok(AudioType::M4a),
            b"M4B" => return Ok(AudioType::M4b),
            b"mp4" => return Ok(AudioType::Mp4),
            _ => {}
        }
    }
    if buffer.len() < SYNC_FRAME_TEST_SIZE {
        return Err(AudioError::NeedMoreHeader(offset + SYNC_FRAME_TEST_SIZE));
    }
    Ok(AudioType::Unknown)
}

fn header_metadata_size(buffer: &[u8], mut offset: usize) -> Result<usize, AudioError> {
    for _ in 0..5 {
        let length = match id3_header_size(buffer, offset)? {
            0 => ape_header_size(buffer, offset)?,
            length => length,
        };
        if length == 0 {
            break;
        }
        offset += length;
    }
    Ok(offset)
}

fn id3_header_size(buffer: &[u8], offset: usize) -> Result<usize, AudioError> {
    const HEADER: usize = 10;
    if buffer.len() < offset + HEADER {
        return Err(AudioError::NeedMoreHeader(offset + HEADER));
    }
    let buffer = &buffer[offset..];
    if buffer.starts_with(b"TAG") {
        return Ok(128);
    }
    if !buffer.starts_with(b"ID3") {
        return Ok(0);
    }
    Ok(HEADER + sync_safe_u32(&buffer[6..10]) as usize)
}

fn ape_header_size(buffer: &[u8], offset: usize) -> Result<usize, AudioError> {
    const HEADER: usize = 32;
    if buffer.len() < offset + HEADER {
        return Err(AudioError::NeedMoreHeader(offset + HEADER));
    }
    let buffer = &buffer[offset..];
    if buffer.starts_with(b"APETAGEX") {
        Ok(HEADER + read_le_u32(&buffer[0x0c..0x10]) as usize)
    } else {
        Ok(0)
    }
}

fn sync_safe_u32(bytes: &[u8]) -> u32 {
    let value = read_be_u32(bytes);
    if value & 0x8080_8080 != 0 {
        return 0;
    }
    ((value & 0xff00_0000) >> 3)
        | ((value & 0x00ff_0000) >> 2)
        | ((value & 0x0000_ff00) >> 1)
        | (value & 0x0000_00ff)
}

fn is_aac(magic: u32) -> bool {
    const AND_MASK: u32 = 0b1111_1111_1111_0110u32 << 16;
    const EXPECTED: u32 = 0b1111_1111_1111_0000u32 << 16;
    magic & AND_MASK == EXPECTED
}

fn is_mp3(buffer: &[u8]) -> bool {
    scan_for_mp3(buffer) >= 3
}

fn scan_for_mp3(buffer: &[u8]) -> usize {
    if buffer.len() < 4 {
        return 0;
    }
    let mut next_frame = vec![0; buffer.len()];
    for index in 0..buffer.len() - 4 {
        if let Some(frame_size) = parse_mp3_header(read_be_u32(&buffer[index..index + 4])) {
            next_frame[index] = index + frame_size;
        }
    }
    let mut result = 0;
    for index in 0..buffer.len() - 4 {
        let mut chain = 0;
        let mut index = index;
        while index < buffer.len() && next_frame[index] != 0 {
            chain += 1;
            index = next_frame[index];
        }
        result = result.max(chain);
    }
    result
}

fn parse_mp3_header(header: u32) -> Option<usize> {
    if (header >> 21) & 0x7ff != 0x7ff {
        return None;
    }
    let version = (header >> 19) & 0b11;
    let layer = (header >> 17) & 0b11;
    let bitrate_index = (header >> 12) & 0b1111;
    let sample_index = (header >> 10) & 0b11;
    if version == 0b01
        || layer == 0
        || bitrate_index == 0
        || bitrate_index == 15
        || sample_index == 3
    {
        return None;
    }
    let bitrate = bitrate_kbps(version, layer, bitrate_index)? * 1000;
    let sample_rate = sample_rate_hz(version, sample_index)?;
    let padding = (header >> 9) & 1;
    Some(match (version, layer) {
        (_, 0b11) => ((12 * bitrate / sample_rate) + padding) * 4,
        (0b11, _) => (144 * bitrate / sample_rate) + padding,
        (_, _) => (72 * bitrate / sample_rate) + padding,
    } as usize)
}

fn bitrate_kbps(version: u32, layer: u32, index: u32) -> Option<u32> {
    let table: &[u32; 14] = match (version, layer) {
        (0b11, 0b11) => &[
            32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
        ],
        (0b11, 0b10) => &[
            32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
        ],
        (0b11, 0b01) => &[
            32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ],
        (_, 0b11) => &[
            32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
        ],
        _ => &[8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
    };
    table.get((index - 1) as usize).copied()
}

fn sample_rate_hz(version: u32, index: u32) -> Option<u32> {
    let table: &[u32; 3] = match version {
        0b11 => &[44_100, 48_000, 32_000],
        0b10 => &[22_050, 24_000, 16_000],
        0b00 => &[11_025, 12_000, 8_000],
        _ => return None,
    };
    table.get(index as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_mp3() -> Vec<u8> {
        let encoded = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/lofty-silent.mp3.b64"
        ))
        .unwrap();
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded.trim()).unwrap()
    }

    fn id3_v2(size: usize) -> Vec<u8> {
        assert!(size < 1 << 28);
        let mut header = vec![b'I', b'D', b'3', 4, 0, 0];
        header.extend_from_slice(&[
            ((size >> 21) & 0x7f) as u8,
            ((size >> 14) & 0x7f) as u8,
            ((size >> 7) & 0x7f) as u8,
            (size & 0x7f) as u8,
        ]);
        header.resize(10 + size, 0);
        header
    }

    #[test]
    fn detects_fixed_magic_and_rejects_short_headers() {
        let mut flac = [0_u8; 32];
        flac[..4].copy_from_slice(b"fLaC");
        assert_eq!(detect_audio_type(&flac), Ok(AudioType::Flac));
        assert_eq!(
            detect_audio_type(b"short"),
            Err(AudioError::NeedMoreHeader(10))
        );
    }

    #[test]
    fn detects_multiple_id3_prefixed_mp3() {
        let mut bytes = id3_v2(0);
        bytes.extend(id3_v2(0));
        bytes.extend(id3_v2(0));
        bytes.extend(fixture_mp3());
        assert_eq!(detect_audio_type(&bytes), Ok(AudioType::Mp3));
    }

    #[test]
    fn detects_large_id3_prefixed_mp3() {
        let mut bytes = id3_v2(4 * 1024);
        bytes.extend(fixture_mp3());
        assert_eq!(detect_audio_type(&bytes), Ok(AudioType::Mp3));
    }

    #[test]
    fn detects_mp4_family_magic() {
        let mut m4a = [0_u8; 32];
        m4a[4..8].copy_from_slice(b"ftyp");
        m4a[8..11].copy_from_slice(b"M4A");
        assert_eq!(detect_audio_type(&m4a), Ok(AudioType::M4a));
        m4a[8..12].copy_from_slice(b"isom");
        assert_eq!(detect_audio_type(&m4a), Ok(AudioType::Mp4));
    }

    #[test]
    fn rejects_junk_and_invalid_mp3_headers() {
        assert_eq!(detect_audio_type(&vec![0; 4096]), Ok(AudioType::Unknown));
        assert_eq!(detect_audio_type(&vec![0xff; 4096]), Ok(AudioType::Unknown));
    }
}
