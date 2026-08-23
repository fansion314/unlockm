//! Bounded standard-audio probing shared by registry and decrypted staging.

use crate::crypto::audio::{AudioType, detect_audio_type};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// The payload window is deliberately bounded.  Tag sizes are only used for
/// checked seeks, never allocations, so a forged ID3/APE size cannot make
/// preflight or history repair retain an unbounded buffer.
const PATH_PROBE_BYTES: usize = 128 * 1024;
const MAX_PREFIX_TAGS: usize = 5;

/// Detect standard audio from an on-disk file. Unlike [`extension`], this can
/// skip a large leading ID3/APE tag with a checked seek before reading the
/// small payload window needed to validate the actual container.
pub fn probe_path(path: &Path) -> std::result::Result<Option<String>, String> {
    let mut file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let length = file
        .metadata()
        .map_err(|error| format!("stat {}: {error}", path.display()))?
        .len();
    let mut offset = 0_u64;
    for _ in 0..MAX_PREFIX_TAGS {
        let mut prefix = [0_u8; 32];
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| format!("seek {}: {error}", path.display()))?;
        let read = file
            .read(&mut prefix)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let tag_size = leading_tag_size(&prefix[..read]);
        let Some(tag_size) = tag_size else { break };
        let Some(next) = offset.checked_add(tag_size) else {
            return Ok(None);
        };
        if next > length {
            return Ok(None);
        }
        offset = next;
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("seek {}: {error}", path.display()))?;
    let mut bytes = vec![0_u8; PATH_PROBE_BYTES];
    let read = file
        .read(&mut bytes)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    bytes.truncate(read);
    Ok(extension(&bytes))
}

fn leading_tag_size(prefix: &[u8]) -> Option<u64> {
    if prefix.starts_with(b"TAG") {
        return Some(128);
    }
    if prefix.len() >= 10 && prefix.starts_with(b"ID3") {
        let size = &prefix[6..10];
        if size.iter().any(|byte| byte & 0x80 != 0) {
            return Some(u64::MAX);
        }
        let value = ((size[0] as u64) << 21)
            | ((size[1] as u64) << 14)
            | ((size[2] as u64) << 7)
            | size[3] as u64;
        return 10_u64.checked_add(value);
    }
    if prefix.len() >= 16 && prefix.starts_with(b"APETAGEX") {
        let size = u32::from_le_bytes(prefix[12..16].try_into().expect("length checked"));
        return 32_u64.checked_add(u64::from(size));
    }
    None
}

pub fn is_standard_audio(header: &[u8]) -> bool {
    detect_audio_type(header).is_ok_and(|kind| kind != AudioType::Unknown)
}

pub fn extension(header: &[u8]) -> Option<String> {
    detect_audio_type(header)
        .ok()
        .and_then(|kind| (kind != AudioType::Unknown).then(|| kind.extension().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn fixture(name: &str) -> Vec<u8> {
        let encoded = std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .unwrap()
    }

    #[test]
    fn adapter_preserves_known_container_extensions() {
        for (fixture_name, expected) in [
            ("lofty-silent.mp3.b64", "mp3"),
            ("lofty-full_test.flac.b64", "flac"),
            ("lofty-full_test.ogg.b64", "ogg"),
            ("lofty-m4a_codec_aac.m4a.b64", "m4a"),
        ] {
            let bytes = fixture(fixture_name);
            assert!(is_standard_audio(&bytes), "{fixture_name}");
            assert_eq!(
                extension(&bytes).as_deref(),
                Some(expected),
                "{fixture_name}"
            );
        }
    }

    #[test]
    fn path_probe_skips_large_id3_without_reading_it_all() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("large-id3.mp3");
        let mut bytes = vec![b'I', b'D', b'3', 4, 0, 0, 0, 1, 0, 0];
        bytes.resize(16 * 1024 + 10, 0);
        bytes.extend(fixture("lofty-silent.mp3.b64"));
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(probe_path(&path).unwrap().as_deref(), Some("mp3"));
    }

    #[test]
    fn path_probe_rejects_truncated_or_forged_tag_lengths() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("forged.mp3");
        std::fs::write(
            &path,
            [b"ID3".as_slice(), &[4, 0, 0, 0x7f, 0x7f, 0x7f, 0x7f]].concat(),
        )
        .unwrap();
        assert_eq!(probe_path(&path).unwrap(), None);
    }
}
