use super::model::{Cover, Metadata, MetadataValue, Provenance};
use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    picture::{Picture, PictureType},
    tag::{Accessor, ItemKey, Tag},
};
use std::{fs, path::Path};

pub fn read(path: &Path) -> std::result::Result<Metadata, String> {
    let file = lofty::read_from_path(path).map_err(|e| e.to_string())?;
    let tag = file.primary_tag().or_else(|| file.first_tag());
    let Some(tag) = tag else {
        return Ok(Metadata::default());
    };
    let text = |value: Option<std::borrow::Cow<'_, str>>| {
        value.map(|v| MetadataValue::new(v.into_owned(), Provenance::ExistingTag))
    };
    Ok(Metadata {
        title: text(tag.title()),
        album: text(tag.album()),
        artists: text(tag.artist()),
        cover: tag.pictures().first().map(|p| Cover {
            bytes: Some(p.data().to_vec()),
            url: None,
            source: Provenance::ExistingTag,
        }),
        duration_ms: u32::try_from(file.properties().duration().as_millis()).ok(),
        ..Default::default()
    })
}
pub fn validate_cover(bytes: &[u8]) -> std::result::Result<(), String> {
    if bytes.len() > 10 * 1024 * 1024 {
        return Err("cover exceeds 10 MiB".into());
    }
    Picture::from_reader(&mut std::io::Cursor::new(bytes))
        .map_err(|_| "cover is not a supported decodable image".into())
        .map(|_| ())
}
pub fn write(
    path: &Path,
    metadata: &Metadata,
    cover: Option<&[u8]>,
) -> std::result::Result<(), String> {
    let before = fs::read(path).map_err(|e| e.to_string())?;
    let mut file = lofty::read_from_path(path).map_err(|e| e.to_string())?;
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
        validate_cover(bytes)?;
        let picture =
            Picture::from_reader(&mut std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            picture.mime_type().cloned(),
            None,
            bytes.to_vec(),
        ));
    }
    file.save_to_path(path, WriteOptions::default())
        .map_err(|e| e.to_string())?;
    let after = fs::read(path).map_err(|e| e.to_string())?;
    if after.is_empty() || before.is_empty() {
        return Err("tag write produced empty audio".into());
    }
    let reread = read(path)?;
    if metadata.title.as_ref().is_some_and(|v| {
        reread
            .title
            .as_ref()
            .map_or(true, |actual| actual.text != v.text)
    }) {
        return Err("tag title did not round-trip".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cover_validation_rejects_bad_or_large_bytes() {
        assert!(validate_cover(b"not-an-image").is_err());
        assert!(validate_cover(&vec![0; 10 * 1024 * 1024 + 1]).is_err());
    }
}
