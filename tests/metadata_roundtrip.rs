use base64::{Engine, engine::general_purpose::STANDARD};
use lofty::{file::AudioFile, properties::FileProperties};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const COVER_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLxmwAAAABJRU5ErkJggg==";
fn silent_mp3() -> Vec<u8> {
    fixture("lofty-silent.mp3.b64")
}
#[test]
fn unsupported_tag_container_is_non_destructive() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("not-audio.flac");
    let bytes = b"fLaC not a valid stream".to_vec();
    fs::write(&path, &bytes).unwrap();
    let metadata = unlockm::metadata::Metadata {
        title: Some(unlockm::metadata::MetadataValue::new(
            "Title",
            unlockm::metadata::Provenance::Container,
        )),
        ..Default::default()
    };
    assert!(unlockm::metadata::tags::write(&path, &metadata, None).is_err());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn native_id3_write_round_trips_without_reencoding_the_mpeg_frame() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("fixture.mp3");
    fs::write(&path, silent_mp3()).unwrap();
    let before = fs::read(&path).unwrap();
    let metadata = unlockm::metadata::Metadata {
        title: Some(unlockm::metadata::MetadataValue::new(
            "Title",
            unlockm::metadata::Provenance::Container,
        )),
        album: Some(unlockm::metadata::MetadataValue::new(
            "Album",
            unlockm::metadata::Provenance::Container,
        )),
        artists: Some(unlockm::metadata::MetadataValue::new(
            "Artist",
            unlockm::metadata::Provenance::Container,
        )),
        ..Default::default()
    };
    unlockm::metadata::tags::write(&path, &metadata, None).unwrap();
    let roundtrip = unlockm::metadata::tags::read(&path).unwrap();
    assert_eq!(roundtrip.title.unwrap().text, "Title");
    let bytes = fs::read(path).unwrap();
    let start = |data: &[u8]| {
        data.windows(2)
            .position(|v| v[0] == 0xff && v[1] & 0xe0 == 0xe0)
            .unwrap()
    };
    assert_eq!(&before[start(&before)..], &bytes[start(&bytes)..]);
}

fn fixture(name: &str) -> Vec<u8> {
    STANDARD
        .decode(
            fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures")
                    .join(name),
            )
            .unwrap()
            .trim(),
        )
        .unwrap()
}
fn properties(path: &Path) -> FileProperties {
    let mut file = fs::File::open(path).unwrap();
    lofty::read_from(&mut file).unwrap().properties().clone()
}
fn audio_signature(
    properties: &FileProperties,
) -> (
    std::time::Duration,
    Option<u32>,
    Option<u32>,
    Option<u8>,
    Option<u8>,
) {
    (
        properties.duration(),
        properties.audio_bitrate(),
        properties.sample_rate(),
        properties.bit_depth(),
        properties.channels(),
    )
}
fn flac_audio(bytes: &[u8]) -> &[u8] {
    assert!(bytes.starts_with(b"fLaC"));
    let mut offset = 4;
    loop {
        let header = bytes[offset];
        let len = ((bytes[offset + 1] as usize) << 16)
            | ((bytes[offset + 2] as usize) << 8)
            | bytes[offset + 3] as usize;
        offset += 4 + len;
        if header & 0x80 != 0 {
            return &bytes[offset..];
        }
    }
}
fn m4a_audio(bytes: &[u8]) -> &[u8] {
    let mut offset = 0;
    while offset + 8 <= bytes.len() {
        let size = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let name = &bytes[offset + 4..offset + 8];
        let (header, total) = if size == 1 {
            let total =
                u64::from_be_bytes(bytes[offset + 8..offset + 16].try_into().unwrap()) as usize;
            (16, total)
        } else {
            (8, size)
        };
        assert!(
            total >= header && offset + total <= bytes.len(),
            "invalid MP4 atom"
        );
        if name == b"mdat" {
            return &bytes[offset + header..offset + total];
        }
        offset += total;
    }
    panic!("M4A fixture had no mdat atom")
}
fn ogg_audio(bytes: &[u8]) -> Vec<u8> {
    let mut offset = 0;
    let mut packet = Vec::new();
    let mut packets = 0;
    let mut audio = Vec::new();
    while offset < bytes.len() {
        assert_eq!(&bytes[offset..offset + 4], b"OggS");
        let segments = bytes[offset + 26] as usize;
        let lacing = &bytes[offset + 27..offset + 27 + segments];
        let mut data = offset + 27 + segments;
        for &len in lacing {
            let end = data + len as usize;
            packet.extend_from_slice(&bytes[data..end]);
            data = end;
            if len < 255 {
                if packets >= 3 {
                    audio.extend_from_slice(&packet);
                }
                packets += 1;
                packet.clear();
            }
        }
        offset = data;
    }
    assert!(packets > 3, "Ogg fixture had no audio packets");
    audio
}
fn audio_fingerprint(extension: &str, bytes: &[u8]) -> [u8; 32] {
    let payload: Vec<u8> = match extension {
        "flac" => flac_audio(bytes).to_vec(),
        "m4a" => m4a_audio(bytes).to_vec(),
        "ogg" => ogg_audio(bytes),
        "mp3" => {
            let start = bytes
                .windows(2)
                .position(|v| v[0] == 0xff && v[1] & 0xe0 == 0xe0)
                .expect("MP3 fixture had no MPEG frame");
            bytes[start..].to_vec()
        }
        _ => unreachable!(),
    };
    Sha256::digest(payload).into()
}

#[test]
fn lofty_real_media_round_trips_when_staged_without_an_extension() {
    let mp3 = silent_mp3();
    for (extension, bytes) in [
        ("mp3", mp3),
        ("flac", fixture("lofty-full_test.flac.b64")),
        ("ogg", fixture("lofty-full_test.ogg.b64")),
        ("m4a", fixture("lofty-m4a_codec_aac.m4a.b64")),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".unlockm-stage-extensionless");
        fs::write(&path, &bytes).unwrap();
        // This is the old production API and documents the regression: Lofty
        // cannot infer the type from the owned extensionless staging name.
        assert!(lofty::read_from_path(&path).is_err(), "{extension}");
        let before_properties = properties(&path);
        let before_payload = audio_fingerprint(extension, &bytes);
        let metadata = unlockm::metadata::Metadata {
            title: Some(unlockm::metadata::MetadataValue::new(
                "Staged Title",
                unlockm::metadata::Provenance::Container,
            )),
            album: Some(unlockm::metadata::MetadataValue::new(
                "Staged Album",
                unlockm::metadata::Provenance::Container,
            )),
            artists: Some(unlockm::metadata::MetadataValue::new(
                "Staged Artist",
                unlockm::metadata::Provenance::Container,
            )),
            ..Default::default()
        };
        let cover = STANDARD.decode(COVER_PNG).unwrap();
        unlockm::metadata::tags::write(&path, &metadata, Some(&cover)).unwrap();
        let reread = unlockm::metadata::tags::read(&path).unwrap();
        assert_eq!(reread.title.unwrap().text, "Staged Title", "{extension}");
        assert_eq!(reread.album.unwrap().text, "Staged Album", "{extension}");
        assert_eq!(reread.artists.unwrap().text, "Staged Artist", "{extension}");
        assert_eq!(reread.cover.unwrap().bytes.unwrap(), cover, "{extension}");
        assert_eq!(
            audio_signature(&properties(&path)),
            audio_signature(&before_properties),
            "{extension} audio properties"
        );
        assert_eq!(
            audio_fingerprint(extension, &fs::read(&path).unwrap()),
            before_payload,
            "{extension}"
        );
    }
}

#[test]
fn lofty_real_media_round_trips_text_picture_and_audio_payload() {
    for (fixture_name, extension) in [
        ("lofty-full_test.flac.b64", "flac"),
        ("lofty-full_test.ogg.b64", "ogg"),
        ("lofty-m4a_codec_aac.m4a.b64", "m4a"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(format!("fixture.{extension}"));
        fs::write(&path, fixture(fixture_name)).unwrap();
        let before = fs::read(&path).unwrap();
        let before_payload = audio_fingerprint(extension, &before);
        let before_properties = properties(&path);
        let metadata = unlockm::metadata::Metadata {
            title: Some(unlockm::metadata::MetadataValue::new(
                "Round Trip Title",
                unlockm::metadata::Provenance::Container,
            )),
            album: Some(unlockm::metadata::MetadataValue::new(
                "Round Trip Album",
                unlockm::metadata::Provenance::Container,
            )),
            artists: Some(unlockm::metadata::MetadataValue::new(
                "Round Trip Artist",
                unlockm::metadata::Provenance::Container,
            )),
            ..Default::default()
        };
        let cover = STANDARD.decode(COVER_PNG).unwrap();
        unlockm::metadata::tags::write(&path, &metadata, Some(&cover)).unwrap();
        let reread = unlockm::metadata::tags::read(&path).unwrap();
        assert_eq!(
            reread.title.unwrap().text,
            "Round Trip Title",
            "{extension}"
        );
        assert_eq!(
            reread.album.unwrap().text,
            "Round Trip Album",
            "{extension}"
        );
        assert_eq!(
            reread.artists.unwrap().text,
            "Round Trip Artist",
            "{extension}"
        );
        assert_eq!(reread.cover.unwrap().bytes.unwrap(), cover, "{extension}");
        // Overall bitrate includes the tag container size and is therefore
        // expected to move. These are decoder-derived audio properties only.
        assert_eq!(
            audio_signature(&properties(&path)),
            audio_signature(&before_properties),
            "{extension} audio properties"
        );
        assert_eq!(
            audio_fingerprint(extension, &fs::read(&path).unwrap()),
            before_payload,
            "{extension} audio payload"
        );
    }
}
