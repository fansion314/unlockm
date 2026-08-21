use super::model::{Cover, Metadata, MetadataValue, Provenance};
use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    picture::{Picture, PictureType},
    tag::{Accessor, ItemKey, Tag},
};
use std::{
    fs::{self, OpenOptions},
    path::Path,
};

pub const MAX_COVER_BYTES: usize = 10 * 1024 * 1024;

/// A local final-output snapshot that keeps useful tag text even when the
/// artwork is malformed or exceeds the bounded history payload limit.
#[derive(Debug, Default)]
pub struct BoundedTagSnapshot {
    pub metadata: Metadata,
    pub artwork_warning: Option<String>,
}

fn probe(path: &Path) -> std::result::Result<lofty::file::TaggedFile, String> {
    let mut source = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| format!("open {} for tag probing: {error}", path.display()))?;
    lofty::read_from(&mut source)
        .map_err(|error| format!("content-probe tags in {}: {error}", path.display()))
}

pub fn read(path: &Path) -> std::result::Result<Metadata, String> {
    let file = probe(path)?;
    let tag = file.primary_tag().or_else(|| file.first_tag());
    let Some(tag) = tag else {
        return Ok(Metadata::default());
    };
    let text = |value: Option<std::borrow::Cow<'_, str>>| {
        value.map(|v| MetadataValue::new(v.into_owned(), Provenance::ExistingTag))
    };
    let cover = tag
        .pictures()
        .first()
        .map(copy_bounded_cover)
        .transpose()?
        .map(|bytes| Cover {
            bytes: Some(bytes),
            url: None,
            source: Provenance::ExistingTag,
        });
    Ok(Metadata {
        title: text(tag.title()),
        album: text(tag.album()),
        artists: text(tag.artist()),
        cover,
        duration_ms: u32::try_from(file.properties().duration().as_millis()).ok(),
        ..Default::default()
    })
}

/// Read a final audio file for history without making bad artwork invalidate
/// otherwise useful text and duration fields. The embedded picture length is
/// checked before any owned history copy is made.
pub fn read_bounded_snapshot(path: &Path) -> std::result::Result<BoundedTagSnapshot, String> {
    let file = probe(path)?;
    let mut metadata = Metadata {
        duration_ms: u32::try_from(file.properties().duration().as_millis()).ok(),
        ..Default::default()
    };
    let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) else {
        return Ok(BoundedTagSnapshot {
            metadata,
            artwork_warning: None,
        });
    };
    let text = |value: Option<std::borrow::Cow<'_, str>>| {
        value.map(|v| MetadataValue::new(v.into_owned(), Provenance::ExistingTag))
    };
    metadata.title = text(tag.title());
    metadata.album = text(tag.album());
    metadata.artists = text(tag.artist());
    let (cover, artwork_warning) = match tag.pictures().first() {
        None => (None, None),
        Some(picture) => match copy_bounded_cover(picture) {
            Ok(bytes) => (
                Some(Cover {
                    bytes: Some(bytes),
                    url: None,
                    source: Provenance::ExistingTag,
                }),
                None,
            ),
            Err(error) => (None, Some(error)),
        },
    };
    metadata.cover = cover;
    Ok(BoundedTagSnapshot {
        metadata,
        artwork_warning,
    })
}
pub fn validate_cover(bytes: &[u8]) -> std::result::Result<(), String> {
    parse_cover(bytes).map(|_| ())
}

fn check_cover_size(length: usize) -> std::result::Result<(), String> {
    if length > MAX_COVER_BYTES {
        Err("cover exceeds 10 MiB".into())
    } else {
        Ok(())
    }
}

fn copy_bounded_cover(picture: &Picture) -> std::result::Result<Vec<u8>, String> {
    // Check the borrowed tag slice before cloning so an oversized embedded
    // picture never becomes a second unbounded allocation in a history/event
    // payload. `Picture::from_reader` below validates only accepted content.
    check_cover_size(picture.data().len())?;
    validate_cover(picture.data())?;
    Ok(picture.data().to_vec())
}

fn parse_cover(bytes: &[u8]) -> std::result::Result<Picture, String> {
    check_cover_size(bytes.len())?;
    Picture::from_reader(&mut std::io::Cursor::new(bytes))
        .map_err(|_| "cover is not a supported decodable image".into())
}
pub fn write(
    path: &Path,
    metadata: &Metadata,
    cover: Option<&[u8]>,
) -> std::result::Result<std::path::PathBuf, String> {
    let candidate = prepare_write(path, metadata, cover)?;
    crate::io::atomic::AtomicOutput::publish_existing_temp(candidate.clone(), path, true).map_err(
        |error| {
            let _ = fs::remove_file(&candidate);
            format!("publish verified metadata candidate: {error}")
        },
    )?;
    Ok(path.to_path_buf())
}

pub(crate) fn prepare_write(
    path: &Path,
    metadata: &Metadata,
    cover: Option<&[u8]>,
) -> std::result::Result<std::path::PathBuf, String> {
    write_with_hook(path, metadata, cover, || Ok(()))
}

pub(crate) fn write_in_place(
    path: &Path,
    metadata: &Metadata,
    cover: Option<&[u8]>,
) -> std::result::Result<(), String> {
    let mut input = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    let mut magic = [0_u8; 4];
    std::io::Read::read_exact(&mut input, &mut magic).map_err(|error| error.to_string())?;
    drop(input);
    if &magic == b"fLaC" {
        if let Some(bytes) = cover {
            validate_cover(bytes)?;
        }
        let before_len = fs::metadata(path).map_err(|error| error.to_string())?.len();
        super::flac::write_in_place(path, metadata, cover)?;
        return verify_written(path, before_len, metadata);
    }
    write_in_place_with_hook(path, metadata, cover, || Ok(()))
}

fn write_in_place_with_hook(
    path: &Path,
    metadata: &Metadata,
    cover: Option<&[u8]>,
    after_save: impl FnOnce() -> std::result::Result<(), String>,
) -> std::result::Result<(), String> {
    write_candidate(path, metadata, cover, after_save)
}

fn write_with_hook<F>(
    path: &Path,
    metadata: &Metadata,
    cover: Option<&[u8]>,
    after_save: F,
) -> std::result::Result<std::path::PathBuf, String>
where
    F: FnOnce() -> std::result::Result<(), String>,
{
    let parent = path.parent().ok_or("staged audio has no parent")?;
    let candidate = tempfile::Builder::new()
        .prefix(".unlockm-metadata-")
        .tempfile_in(parent)
        .map_err(|error| format!("create metadata candidate: {error}"))?
        .into_temp_path()
        .keep()
        .map_err(|error| format!("keep metadata candidate: {}", error.error))?;
    if let Err(error) = fs::copy(path, &candidate) {
        let _ = fs::remove_file(&candidate);
        return Err(format!("copy staged audio to metadata candidate: {error}"));
    }
    let result = write_candidate(&candidate, metadata, cover, after_save);
    if let Err(error) = result {
        let _ = fs::remove_file(&candidate);
        return Err(error);
    }
    Ok(candidate)
}

fn write_candidate(
    path: &Path,
    metadata: &Metadata,
    cover: Option<&[u8]>,
    after_save: impl FnOnce() -> std::result::Result<(), String>,
) -> std::result::Result<(), String> {
    let before_len = fs::metadata(path).map_err(|e| e.to_string())?.len();
    let mut file = probe(path)?;
    let tag_type = file.primary_tag_type();
    if file.primary_tag_mut().is_none() {
        file.insert_tag(Tag::new(tag_type));
    }
    let tag = if let Some(tag) = file.primary_tag_mut() {
        tag
    } else {
        file.first_tag_mut().ok_or("format has no writable tag")?
    };
    if let Some(value) = &metadata.title {
        tag.insert_text(ItemKey::TrackTitle, value.text.clone());
    }
    if let Some(value) = &metadata.album {
        tag.insert_text(ItemKey::AlbumTitle, value.text.clone());
    }
    if let Some(value) = &metadata.artists {
        tag.insert_text(ItemKey::TrackArtist, value.text.clone());
    }
    if let Some(bytes) = cover {
        let mut picture = parse_cover(bytes)?;
        picture.set_pic_type(PictureType::CoverFront);
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(picture);
    }
    let mut output = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("open {} for tag write: {error}", path.display()))?;
    file.save_to(&mut output, WriteOptions::default())
        .map_err(|e| e.to_string())?;
    after_save()?;
    verify_written(path, before_len, metadata)
}

fn verify_written(
    path: &Path,
    before_len: u64,
    metadata: &Metadata,
) -> std::result::Result<(), String> {
    let after_len = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if after_len == 0 || before_len == 0 {
        return Err("tag write produced empty audio".into());
    }
    let reread = read(path)?;
    if metadata.title.as_ref().is_some_and(|v| {
        reread
            .title
            .as_ref()
            .is_none_or(|actual| actual.text != v.text)
    }) {
        return Err("tag title did not round-trip".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};

    fn flac_audio(bytes: &[u8]) -> &[u8] {
        assert_eq!(&bytes[..4], b"fLaC");
        let mut offset = 4;
        loop {
            let last = bytes[offset] & 0x80 != 0;
            let length =
                u32::from_be_bytes([0, bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
                    as usize;
            offset += 4 + length;
            if last {
                return &bytes[offset..];
            }
        }
    }
    #[test]
    fn cover_validation_rejects_bad_or_large_bytes() {
        assert!(validate_cover(b"not-an-image").is_err());
        assert!(validate_cover(&vec![0; 10 * 1024 * 1024 + 1]).is_err());
    }
    #[test]
    fn valid_tagless_audio_is_an_empty_metadata_value() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".unlockm-stage-tagless");
        let fixture = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lofty-silent.mp3.b64"),
        )
        .unwrap();
        std::fs::write(&path, STANDARD.decode(fixture.trim()).unwrap()).unwrap();
        let metadata = read(&path).unwrap();
        assert!(metadata.title.is_none());
        assert!(metadata.album.is_none());
        assert!(metadata.artists.is_none());
        assert!(metadata.cover.is_none());
    }
    #[test]
    fn post_save_failure_preserves_original_stage_and_cleans_candidate() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".unlockm-stage-original");
        let fixture = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lofty-silent.mp3.b64"),
        )
        .unwrap();
        std::fs::write(&path, STANDARD.decode(fixture.trim()).unwrap()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let metadata = Metadata {
            title: Some(MetadataValue::new("mutated", Provenance::Container)),
            ..Default::default()
        };
        assert!(
            write_with_hook(&path, &metadata, None, || {
                Err("injected post-save verification failure".into())
            })
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(probe(&path).is_ok());
        assert_eq!(
            std::fs::read_dir(temp.path())
                .unwrap()
                .filter_map(std::result::Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(".unlockm-"))
                .count(),
            1
        );
    }
    #[test]
    fn in_place_post_save_failure_is_reported_after_private_stage_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".unlockm-stage-disposable");
        let fixture = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lofty-silent.mp3.b64"),
        )
        .unwrap();
        std::fs::write(&path, STANDARD.decode(fixture.trim()).unwrap()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let metadata = Metadata {
            title: Some(MetadataValue::new("mutated", Provenance::Container)),
            ..Default::default()
        };
        assert!(
            write_in_place_with_hook(&path, &metadata, None, || {
                Err("injected post-save verification failure".into())
            })
            .is_err()
        );
        assert_ne!(std::fs::read(&path).unwrap(), before);
        assert_eq!(read(&path).unwrap().title.unwrap().text, "mutated");
    }
    #[test]
    fn streaming_flac_write_grows_and_reuses_padding_without_changing_audio() {
        const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLxmwAAAABJRU5ErkJggg==";
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".unlockm-stage-flac");
        let fixture = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lofty-full_test.flac.b64"),
        )
        .unwrap();
        std::fs::write(&path, STANDARD.decode(fixture.trim()).unwrap()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let before_audio = flac_audio(&before).to_vec();

        let long_title = "x".repeat(64 * 1024);
        let metadata = Metadata {
            title: Some(MetadataValue::new(
                long_title.clone(),
                Provenance::Container,
            )),
            album: Some(MetadataValue::new("album", Provenance::Container)),
            artists: Some(MetadataValue::new("artist", Provenance::Container)),
            ..Default::default()
        };
        write_in_place(&path, &metadata, Some(&STANDARD.decode(PNG).unwrap())).unwrap();
        let grown = std::fs::read(&path).unwrap();
        assert_eq!(flac_audio(&grown), before_audio);
        let reread = read(&path).unwrap();
        assert_eq!(reread.title.unwrap().text, long_title);
        assert_eq!(reread.album.unwrap().text, "album");
        assert_eq!(reread.artists.unwrap().text, "artist");
        assert_eq!(
            reread.cover.unwrap().bytes.unwrap(),
            STANDARD.decode(PNG).unwrap()
        );

        let short = Metadata {
            title: Some(MetadataValue::new("short", Provenance::Container)),
            ..Default::default()
        };
        write_in_place(&path, &short, None).unwrap();
        let padded = std::fs::read(&path).unwrap();
        assert_eq!(flac_audio(&padded), before_audio);
        let reread = read(&path).unwrap();
        assert_eq!(reread.title.unwrap().text, "short");
        assert_eq!(
            reread.cover.unwrap().bytes.unwrap(),
            STANDARD.decode(PNG).unwrap()
        );
    }
    #[test]
    fn copy_failure_cleans_candidate_immediately() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join(".unlockm-stage-directory");
        std::fs::create_dir(&source).unwrap();
        let metadata = Metadata {
            title: Some(MetadataValue::new("mutated", Provenance::Container)),
            ..Default::default()
        };
        assert!(prepare_write(&source, &metadata, None).is_err());
        assert_eq!(
            std::fs::read_dir(temp.path())
                .unwrap()
                .filter_map(std::result::Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".unlockm-metadata-"))
                .count(),
            0
        );
    }

    #[test]
    fn oversized_cover_is_rejected_before_any_owned_snapshot_copy() {
        assert_eq!(check_cover_size(MAX_COVER_BYTES), Ok(()));
        assert_eq!(
            check_cover_size(MAX_COVER_BYTES + 1).unwrap_err(),
            "cover exceeds 10 MiB"
        );
        let picture = Picture::unchecked(vec![0_u8; MAX_COVER_BYTES + 1]).build();
        assert_eq!(
            copy_bounded_cover(&picture).unwrap_err(),
            "cover exceeds 10 MiB"
        );
    }

    #[test]
    fn oversized_embedded_cover_is_not_copied_into_metadata_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("oversized-cover.mp3");
        let fixture = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lofty-silent.mp3.b64"),
        )
        .unwrap();
        std::fs::write(&path, STANDARD.decode(fixture.trim()).unwrap()).unwrap();

        let width = 2048_u32;
        let height = 2048_u32;
        let image_bytes = usize::try_from(width * height * 3).unwrap();
        assert!(image_bytes > MAX_COVER_BYTES);
        let file_size = 54 + image_bytes;
        let mut bmp = vec![0_u8; file_size];
        bmp[..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&width.to_le_bytes());
        bmp[22..26].copy_from_slice(&height.to_le_bytes());
        bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
        bmp[34..38].copy_from_slice(&(image_bytes as u32).to_le_bytes());

        let mut file = probe(&path).unwrap();
        let tag_type = file.primary_tag_type();
        if file.primary_tag_mut().is_none() {
            file.insert_tag(Tag::new(tag_type));
        }
        let tag = file.primary_tag_mut().unwrap();
        tag.insert_text(ItemKey::TrackTitle, "retained title".into());
        tag.push_picture(Picture::from_reader(&mut std::io::Cursor::new(bmp)).unwrap());
        let mut output = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.save_to(&mut output, WriteOptions::default()).unwrap();

        assert_eq!(read(&path).unwrap_err(), "cover exceeds 10 MiB");
        let snapshot = read_bounded_snapshot(&path).unwrap();
        assert_eq!(snapshot.metadata.title.unwrap().text, "retained title");
        assert!(snapshot.metadata.cover.is_none());
        assert_eq!(
            snapshot.artwork_warning.as_deref(),
            Some("cover exceeds 10 MiB")
        );
    }
}
