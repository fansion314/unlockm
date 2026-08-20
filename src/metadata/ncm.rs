use super::model::{Cover, Metadata, MetadataValue, Provenance};
use std::{fs::File, path::Path};

pub fn parse(path: &Path) -> std::result::Result<Metadata, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let ncm = umc_ncm::header::NCMFile::new_from_readable(&mut file).map_err(|e| e.to_string())?;
    let json: serde_json::Value =
        serde_json::from_slice(&ncm.get_metadata().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    Ok(from_json(json, ncm.image1))
}

pub fn from_json(json: serde_json::Value, image1: Option<Vec<u8>>) -> Metadata {
    let text = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| json.get(*key).and_then(serde_json::Value::as_str))
            .map(|v| MetadataValue::new(v, Provenance::Container))
    };
    let artists = json
        .get("artist")
        .or_else(|| json.get("artists"))
        .and_then(|items| items.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .or_else(|| item.get("name").and_then(serde_json::Value::as_str))
                })
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .filter(|v| !v.is_empty())
        .map(|v| MetadataValue::new(v, Provenance::Container));
    let album = json
        .get("album")
        .and_then(|v| {
            v.as_str()
                .or_else(|| v.get("name").and_then(serde_json::Value::as_str))
        })
        .or_else(|| json.get("albumName").and_then(serde_json::Value::as_str))
        .map(|v| MetadataValue::new(v, Provenance::Container));
    let cover = image1
        .map(|bytes| Cover {
            bytes: Some(bytes),
            url: None,
            source: Provenance::Container,
        })
        .or_else(|| {
            json.get("albumPic")
                .or_else(|| json.get("albumPicUrl"))
                .and_then(serde_json::Value::as_str)
                .map(|url| Cover {
                    bytes: None,
                    url: Some(url.to_owned()),
                    source: Provenance::Container,
                })
        });
    Metadata {
        title: text(&["musicName", "name", "title"]),
        album,
        artists,
        duration_ms: json
            .get("duration")
            .or_else(|| json.get("durationMs"))
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as u32),
        provider_id: json
            .get("musicId")
            .or_else(|| json.get("songId"))
            .and_then(serde_json::Value::as_i64)
            .map(|v| v.to_string()),
        cover,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn music_and_dj_shapes_keep_container_precedence() {
        let music = from_json(
            serde_json::json!({"musicName":"Song","artist":["Artist"],"album":"Album","musicId":1,"duration":1234,"albumPicUrl":"https://cover"}),
            None,
        );
        assert_eq!(music.title.unwrap().text, "Song");
        assert_eq!(music.artists.unwrap().text, "Artist");
        let dj = from_json(
            serde_json::json!({"name":"Talk","artists":[{"name":"Host"}],"album":{"name":"Series"},"songId":2}),
            Some(vec![1, 2]),
        );
        assert_eq!(dj.album.unwrap().text, "Series");
        assert_eq!(dj.cover.unwrap().bytes, Some(vec![1, 2]));
    }
}
