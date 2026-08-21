//! Bounded standard-audio probing shared by registry and decrypted staging.

use crate::crypto::audio::{detect_audio_type, AudioType};

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
}
