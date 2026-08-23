//! Range-aware, item-ID-only audio delivery for the desktop webview.

use crate::history::HistoryStore;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    sync::Arc,
};
use tauri::http::{
    Method, Request, Response, StatusCode,
    header::{
        ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, CONTENT_LENGTH,
        CONTENT_RANGE, CONTENT_TYPE, RANGE,
    },
};

const MAX_RANGE_BYTES: u64 = 1_000 * 1024;

pub fn response(history: &Arc<HistoryStore>, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    match try_response(history, &request) {
        Ok(response) => response,
        Err(status) => response_builder(status).body(Vec::new()).unwrap(),
    }
}

fn try_response(
    history: &Arc<HistoryStore>,
    request: &Request<Vec<u8>>,
) -> Result<Response<Vec<u8>>, StatusCode> {
    if !matches!(request.method(), &Method::GET | &Method::HEAD) {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }
    let item_id = parse_item_id(request.uri().path()).ok_or(StatusCode::NOT_FOUND)?;
    let path = history
        .music_output_path(item_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        std::io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StatusCode::FORBIDDEN);
    }
    let len = metadata.len();
    if len == 0 {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    let format = unlockm::formats::audio::probe_path(&path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?;
    let mime = audio_mime(&format).ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?;
    let mut file = fs::File::open(&path).map_err(|_| StatusCode::FORBIDDEN)?;

    if request.method() == Method::HEAD {
        return response_builder(StatusCode::OK)
            .header(CONTENT_TYPE, mime)
            .header(CONTENT_LENGTH, len)
            .body(Vec::new())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }

    let requested = match request
        .headers()
        .get(RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|value| parse_range(value, len))
        .transpose()
    {
        Ok(value) => value.unwrap_or((0, len - 1)),
        Err(_) => {
            return response_builder(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(CONTENT_RANGE, format!("bytes */{len}"))
                .body(Vec::new())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let start = requested.0;
    let end = requested.1.min(start.saturating_add(MAX_RANGE_BYTES - 1));
    let bytes_to_read = end - start + 1;
    let mut body = Vec::with_capacity(bytes_to_read as usize);
    file.seek(SeekFrom::Start(start))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    file.take(bytes_to_read)
        .read_to_end(&mut body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    response_builder(StatusCode::PARTIAL_CONTENT)
        .header(CONTENT_TYPE, mime)
        .header(CONTENT_LENGTH, body.len())
        .header(CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn response_builder(status: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(status)
        .header(ACCEPT_RANGES, "bytes")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            ACCESS_CONTROL_EXPOSE_HEADERS,
            "content-range, accept-ranges",
        )
}

fn parse_item_id(path: &str) -> Option<i64> {
    let value = path
        .strip_prefix("/item/")
        .or_else(|| path.strip_prefix('/'))?;
    if value.is_empty() || value.contains('/') {
        return None;
    }
    value.parse::<i64>().ok().filter(|value| *value > 0)
}

fn parse_range(value: &str, len: u64) -> Result<(u64, u64), StatusCode> {
    let value = value
        .strip_prefix("bytes=")
        .ok_or(StatusCode::RANGE_NOT_SATISFIABLE)?;
    if value.contains(',') {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    let (start, end) = value
        .split_once('-')
        .ok_or(StatusCode::RANGE_NOT_SATISFIABLE)?;
    if start.is_empty() {
        let suffix = end
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or(StatusCode::RANGE_NOT_SATISFIABLE)?;
        let start = len.saturating_sub(suffix.min(len));
        return Ok((start, len - 1));
    }
    let start = start
        .parse::<u64>()
        .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?;
    let end = if end.is_empty() {
        len - 1
    } else {
        end.parse::<u64>()
            .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?
    };
    if start >= len || end < start {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    Ok((start, end.min(len - 1)))
}

fn audio_mime(format: &str) -> Option<&'static str> {
    match format {
        "mp3" => Some("audio/mpeg"),
        "flac" => Some("audio/flac"),
        "ogg" => Some("audio/ogg"),
        "m4a" | "m4b" | "mp4" => Some("audio/mp4"),
        "aac" => Some("audio/aac"),
        "wav" => Some("audio/wav"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{CreateRun, InputKind, PersistedItemInput, RunSettings, StoredItemKind};
    use base64::{Engine, engine::general_purpose::STANDARD};

    fn history_with_audio() -> (tempfile::TempDir, Arc<HistoryStore>, i64, Vec<u8>) {
        let directory = tempfile::tempdir().unwrap();
        let audio = STANDARD
            .decode(include_str!("../../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        let output = directory.path().join("track-without-extension");
        fs::write(&output, &audio).unwrap();
        let history =
            Arc::new(HistoryStore::open(directory.path().join("history.sqlite")).unwrap());
        let run = history
            .create_run(CreateRun {
                input_kind: InputKind::File,
                input_path: directory.path().join("track.ncm"),
                output_path: directory.path().to_path_buf(),
                settings: RunSettings {
                    update_metadata: false,
                    copy_passthrough: true,
                    overwrite: false,
                },
                total_items: 1,
            })
            .unwrap();
        let item_id = history
            .record_item(
                run.id,
                PersistedItemInput {
                    ordinal: 0,
                    item_kind: StoredItemKind::Music,
                    source_path: directory.path().join("track.ncm"),
                    relative_path: "track.ncm".into(),
                    destination_path: Some(output),
                    action: Some("convert".into()),
                    outcome: "converted".into(),
                    logical_bytes: 1,
                    output_bytes: Some(audio.len() as u64),
                    output_format: Some("mp3".into()),
                    bitrate_kbps: None,
                    preflight_ms: 0,
                    decrypt_ms: 0,
                    metadata_resolve_ms: 0,
                    tag_write_ms: 0,
                    fallback_ms: 0,
                    copy_ms: 0,
                    publish_ms: 0,
                    error_code: None,
                    error_detail: None,
                    warnings: Vec::new(),
                    title: Some("Track".into()),
                    artist: None,
                    album: None,
                    duration_ms: None,
                    artwork: None,
                },
            )
            .unwrap();
        (directory, history, item_id, audio)
    }

    #[test]
    fn head_and_bounded_range_responses_hide_the_path() {
        let (_directory, history, item_id, audio) = history_with_audio();
        let head = Request::builder()
            .method(Method::HEAD)
            .uri(format!("unlockm-audio://localhost/item/{item_id}"))
            .body(Vec::new())
            .unwrap();
        let head_response = response(&history, head);
        assert_eq!(head_response.status(), StatusCode::OK);
        assert_eq!(head_response.headers()[CONTENT_TYPE], "audio/mpeg");
        assert_eq!(
            head_response.headers()[CONTENT_LENGTH],
            audio.len().to_string()
        );

        let range = Request::builder()
            .method(Method::GET)
            .uri(format!("unlockm-audio://localhost/item/{item_id}"))
            .header(RANGE, "bytes=4-19")
            .body(Vec::new())
            .unwrap();
        let range_response = response(&history, range);
        assert_eq!(range_response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(range_response.body(), &audio[4..20]);
        assert_eq!(
            range_response.headers()[CONTENT_RANGE],
            format!("bytes 4-19/{}", audio.len())
        );
    }

    #[test]
    fn invalid_unknown_and_multi_ranges_are_rejected_without_details() {
        let (_directory, history, item_id, _audio) = history_with_audio();
        for (uri, range, expected) in [
            (
                "unlockm-audio://localhost/item/nope".to_owned(),
                None,
                StatusCode::NOT_FOUND,
            ),
            (
                "unlockm-audio://localhost/item/99999".to_owned(),
                None,
                StatusCode::NOT_FOUND,
            ),
            (
                format!("unlockm-audio://localhost/item/{item_id}"),
                Some("bytes=0-1,4-5"),
                StatusCode::RANGE_NOT_SATISFIABLE,
            ),
        ] {
            let mut request = Request::builder().method(Method::GET).uri(uri);
            if let Some(range) = range {
                request = request.header(RANGE, range);
            }
            let response = response(&history, request.body(Vec::new()).unwrap());
            assert_eq!(response.status(), expected);
            assert!(response.body().is_empty());
        }
    }

    #[test]
    fn copied_music_is_playable_but_deleted_or_unknown_output_is_not() {
        let (directory, history, item_id, _audio) = history_with_audio();
        history
            .execute_test_sql(&format!(
                "UPDATE items SET outcome = 'copied' WHERE id = {item_id}"
            ))
            .unwrap();
        let request = Request::builder()
            .method(Method::HEAD)
            .uri(format!("unlockm-audio://localhost/item/{item_id}"))
            .body(Vec::new())
            .unwrap();
        assert_eq!(response(&history, request).status(), StatusCode::OK);

        let output = directory.path().join("track-without-extension");
        fs::write(&output, b"not music").unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("unlockm-audio://localhost/item/{item_id}"))
            .body(Vec::new())
            .unwrap();
        assert_eq!(
            response(&history, request).status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );

        fs::remove_file(&output).unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("unlockm-audio://localhost/item/{item_id}"))
            .body(Vec::new())
            .unwrap();
        assert_eq!(response(&history, request).status(), StatusCode::NOT_FOUND);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_recorded_outputs_are_forbidden() {
        use std::os::unix::fs::symlink;

        let (directory, history, item_id, _audio) = history_with_audio();
        let output = directory.path().join("track-without-extension");
        fs::remove_file(&output).unwrap();
        symlink(directory.path().join("history.sqlite"), &output).unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("unlockm-audio://localhost/item/{item_id}"))
            .body(Vec::new())
            .unwrap();
        assert_eq!(response(&history, request).status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn deleted_history_record_immediately_revokes_audio_access() {
        let (_directory, history, item_id, _audio) = history_with_audio();
        let run_id = history.list_runs(None, 1).unwrap().runs[0].id;
        history.delete_run(run_id).unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("unlockm-audio://localhost/item/{item_id}"))
            .body(Vec::new())
            .unwrap();
        assert_eq!(response(&history, request).status(), StatusCode::NOT_FOUND);
    }
}
