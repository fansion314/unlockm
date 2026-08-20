//! Evidence-gated metadata. A conversion is already complete before this
//! module runs; parser, provider, and tag failures are warnings.
pub mod merge;
pub mod model;
pub mod ncm;
pub mod provider;
pub mod tags;
use crate::formats::adapter::MetadataHints;
use crate::formats::Kind;
pub use merge::merge;
pub use model::{Metadata, MetadataValue, Provenance};
use provider::qq::MetadataClient;
use std::path::Path;

pub fn enrich_staged(
    audio: &Path,
    source: &Path,
    kind: Kind,
    hints: &MetadataHints,
    client: &MetadataClient,
) -> std::result::Result<Option<std::path::PathBuf>, String> {
    let existing = tags::read(audio).map_err(|error| {
        format!(
            "read existing tags from staged {}: {error}",
            audio.display()
        )
    })?;
    let incoming = match kind {
        Kind::Ncm => ncm::parse(source)?,
        Kind::QmcV2 => qmc_incoming(&existing, source, hints, client)?,
        _ => Metadata::default(),
    };
    let merged = merge(existing, incoming);
    apply_merged(audio, merged, client)
        .map_err(|error| format!("update staged metadata at {}: {error}", audio.display()))
}

fn apply_merged(
    audio: &Path,
    merged: Metadata,
    client: &MetadataClient,
) -> std::result::Result<Option<std::path::PathBuf>, String> {
    if merged.is_empty() {
        return Ok(None);
    }
    let cover = merged
        .cover
        .as_ref()
        .and_then(|cover| {
            if let Some(bytes) = &cover.bytes {
                Some(Ok(bytes.clone()))
            } else {
                cover.url.as_deref().map(|url| client.cover(url))
            }
        })
        .transpose()?;
    tags::prepare_write(audio, &merged, cover.as_deref()).map(Some)
}

fn qmc_incoming(
    existing: &Metadata,
    source: &Path,
    hints: &MetadataHints,
    client: &MetadataClient,
) -> std::result::Result<Metadata, String> {
    // A footer identity is sufficient on its own. Do not fall back to a fuzzy
    // query after an exact endpoint fails: failure must leave the audio alone.
    if let Some(identity) = hints.media_mid.as_deref().or(hints.resource_id.as_deref()) {
        return client
            .exact(identity)
            .map(|metadata| metadata.with_provenance(Provenance::ExactProvider));
    }
    let (title, artists) = search_hints(existing, source);
    let Some((title, artists)) = title.zip(artists) else {
        return Ok(Metadata::default());
    };
    client
        .search_verified(&title, &artists, existing.duration_ms)
        .map(|metadata| metadata.map(|value| value.with_provenance(Provenance::VerifiedSearch)))
        .map(|metadata| metadata.unwrap_or_default())
}

fn search_hints(existing: &Metadata, source: &Path) -> (Option<String>, Option<String>) {
    let title = existing.title.as_ref().map(|value| value.text.clone());
    let artists = existing.artists.as_ref().map(|value| value.text.clone());
    if title.is_some() && artists.is_some() {
        return (title, artists);
    }
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let Some((filename_artist, filename_title)) = stem.split_once(" - ") else {
        return (title, artists);
    };
    (
        title.or_else(|| Some(filename_title.to_owned())),
        artists.or_else(|| Some(filename_artist.to_owned())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        thread,
    };

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLxmwAAAABJRU5ErkJggg==";
    fn server(ambiguous: bool, expected_requests: usize) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        thread::spawn(move || {
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let size = stream.read(&mut request).unwrap();
                let route = std::str::from_utf8(&request[..size])
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/");
                let body = if route.starts_with("/exact/") {
                    format!(r#"{{"title":{{"text":"Exact Song","source":"FilenameHint","confidence":1}},"album":{{"text":"Exact Album","source":"FilenameHint","confidence":1}},"artists":{{"text":"Exact Artist","source":"FilenameHint","confidence":1}},"duration_ms":1000,"provider_id":"42","cover":{{"bytes":null,"url":"http://{address}/cover","source":"FilenameHint"}}}}"#).into_bytes()
                } else if route == "/cover" {
                    STANDARD.decode(PNG).unwrap()
                } else if ambiguous {
                    br#"[{"title":"Song","artists":"Artist","duration_ms":1000,"metadata":{"title":{"text":"Wrong","source":"FilenameHint","confidence":1},"album":null,"artists":null,"duration_ms":1000,"provider_id":null,"cover":null}},{"title":"Song","artists":"Artist","duration_ms":1000,"metadata":{"title":{"text":"Wrong","source":"FilenameHint","confidence":1},"album":null,"artists":null,"duration_ms":1000,"provider_id":null,"cover":null}}]"#.to_vec()
                } else {
                    b"[]".to_vec()
                };
                observed.fetch_add(1, Ordering::SeqCst);
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        (format!("http://{address}"), calls)
    }
    fn staged_audio() -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let audio = temp.path().join("Artist - Song.mp3");
        let fixture = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lofty-silent.mp3.b64"),
        )
        .unwrap();
        std::fs::write(&audio, STANDARD.decode(fixture.trim()).unwrap()).unwrap();
        (temp, audio)
    }
    #[test]
    fn qmc_exact_metadata_cover_and_cache_use_test_only_loopback_transport() {
        let (_temp, audio) = staged_audio();
        let (base, calls) = server(false, 2);
        let client = MetadataClient::with_test_base(1, base);
        let candidate = enrich_staged(
            &audio,
            Path::new("unrelated.mflac"),
            Kind::QmcV2,
            &MetadataHints {
                resource_id: Some("42".into()),
                media_mid: Some("mid".into()),
            },
            &client,
        )
        .unwrap()
        .unwrap();
        crate::io::atomic::AtomicOutput::publish_existing_temp(candidate, &audio, true).unwrap();
        client.exact("mid").unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let result = tags::read(&audio).unwrap();
        assert_eq!(result.title.unwrap().text, "Exact Song");
        assert_eq!(
            result.cover.unwrap().bytes.unwrap(),
            STANDARD.decode(PNG).unwrap()
        );
    }
    #[test]
    fn qmc_ambiguous_search_remains_non_mutating_with_test_only_loopback_transport() {
        let (_temp, audio) = staged_audio();
        let (base, calls) = server(true, 1);
        let client = MetadataClient::with_test_base(1, base);
        enrich_staged(
            &audio,
            Path::new("Artist - Song.mflac"),
            Kind::QmcV2,
            &MetadataHints::default(),
            &client,
        )
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(tags::read(&audio).unwrap().title.is_none());
    }
    #[test]
    fn rejected_cover_leaves_staged_audio_unchanged_for_best_effort_pipeline() {
        let (_temp, audio) = staged_audio();
        let before = std::fs::read(&audio).unwrap();
        let metadata = Metadata {
            title: Some(MetadataValue::new("Title", Provenance::Container)),
            cover: Some(model::Cover {
                bytes: None,
                url: Some("http://127.0.0.1/cover".into()),
                source: Provenance::Container,
            }),
            ..Default::default()
        };
        assert!(apply_merged(&audio, metadata, &MetadataClient::new(1)).is_err());
        assert_eq!(std::fs::read(&audio).unwrap(), before);
    }
}
