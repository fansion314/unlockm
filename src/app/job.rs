use super::{
    JobTimings, Summary,
    events::{ArtworkSnapshot, MusicSnapshot, ReportWarning},
};
use crate::app::runner::Cancellation;
use crate::{
    config::RunConfig,
    formats::{self, PreparedClassification},
    io::atomic::AtomicOutput,
    keys::KeySources,
    metadata::{self, provider::qq::MetadataClient},
    pathing::{Reservations, plan},
};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime},
};

#[derive(Debug)]
pub enum PlannedAction {
    Copy,
    Decrypt {
        prepared: formats::adapter::PreparedDecrypt,
        extension: String,
    },
}
#[derive(Debug)]
pub struct PlannedJob {
    pub source: std::path::PathBuf,
    pub destination: std::path::PathBuf,
    pub action: PlannedAction,
}
#[derive(Debug)]
pub struct StagedDecrypt {
    pub path: std::path::PathBuf,
}

/// The execution result stays separate from [`Summary`] so host observers can
/// retain non-fatal metadata diagnostics without scraping tracing output.
#[derive(Debug, Default)]
pub struct ExecutionReport {
    pub summary: Summary,
    pub warnings: Vec<ReportWarning>,
    pub music: Option<MusicSnapshot>,
}

fn fallback_music_snapshot(source: &Path) -> MusicSnapshot {
    let stem = source
        .file_stem()
        .map(|value| value.to_string_lossy().trim().to_owned())
        .filter(|value| !value.is_empty());
    let artist_title = stem.as_deref().and_then(|stem| {
        let (artist, title) = stem.split_once(" - ")?;
        let artist = artist.trim();
        let title = title.trim();
        (!artist.is_empty() && !title.is_empty()).then(|| (artist.to_owned(), title.to_owned()))
    });
    match artist_title {
        Some((artist, title)) => MusicSnapshot {
            title: Some(title),
            artist: Some(artist),
            ..Default::default()
        },
        None => MusicSnapshot {
            title: stem,
            ..Default::default()
        },
    }
}

fn artwork_mime_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

fn snapshot_music(
    source: &Path,
    destination: &Path,
    source_kind: Option<formats::Kind>,
    warnings: &mut Vec<ReportWarning>,
) -> MusicSnapshot {
    let mut snapshot = fallback_music_snapshot(source);
    match output_facts(destination) {
        Ok((bytes, format)) => {
            snapshot.output_bytes = Some(bytes);
            snapshot.output_format = format;
        }
        Err(detail) => warnings.push(ReportWarning {
            code: "output_snapshot_unavailable".into(),
            detail,
        }),
    }
    let bounded = match crate::metadata::tags::read_bounded_snapshot(destination) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            warnings.push(ReportWarning {
                code: "metadata_snapshot_unavailable".into(),
                detail: error,
            });
            return snapshot;
        }
    };
    if let Some(detail) = bounded.artwork_warning {
        warnings.push(ReportWarning {
            code: "artwork_snapshot_unavailable".into(),
            detail,
        });
    }
    let metadata = bounded.metadata;
    snapshot.title = metadata.title.map(|value| value.text).or(snapshot.title);
    snapshot.artist = metadata.artists.map(|value| value.text).or(snapshot.artist);
    snapshot.album = metadata.album.map(|value| value.text).or(snapshot.album);
    snapshot.duration_ms = metadata.duration_ms.or(snapshot.duration_ms);
    if let Some(bytes) = metadata.cover.and_then(|cover| cover.bytes) {
        if let Some(mime_type) = artwork_mime_type(&bytes) {
            snapshot.artwork = Some(ArtworkSnapshot {
                mime_type: mime_type.into(),
                bytes,
            });
        } else {
            warnings.push(ReportWarning {
                code: "artwork_snapshot_unavailable".into(),
                detail: "embedded cover has an unsupported image format".into(),
            });
        }
    }
    if source_kind == Some(formats::Kind::Ncm)
        && let Ok(container) = crate::metadata::ncm::parse(source)
    {
        snapshot.title = container.title.map(|value| value.text).or(snapshot.title);
        snapshot.artist = container
            .artists
            .map(|value| value.text)
            .or(snapshot.artist);
        snapshot.album = container.album.map(|value| value.text).or(snapshot.album);
        snapshot.duration_ms = container.duration_ms.or(snapshot.duration_ms);
        if snapshot.artwork.is_none()
            && let Some(bytes) = container.cover.and_then(|cover| cover.bytes)
            && let Some(mime_type) = artwork_mime_type(&bytes)
        {
            snapshot.artwork = Some(ArtworkSnapshot {
                mime_type: mime_type.into(),
                bytes,
            });
        }
    }
    snapshot
}

fn output_facts(destination: &Path) -> Result<(u64, Option<String>), String> {
    let output_bytes = fs::metadata(destination)
        .map_err(|error| format!("stat {}: {error}", destination.display()))?
        .len();
    let mut file = fs::File::open(destination)
        .map_err(|error| format!("open {} for format probe: {error}", destination.display()))?;
    let mut header = vec![0_u8; 8192];
    let read = file
        .read(&mut header)
        .map_err(|error| format!("read {} for format probe: {error}", destination.display()))?;
    header.truncate(read);
    Ok((output_bytes, formats::extension(&header)))
}

fn music_execution_report(
    summary: Summary,
    source: &Path,
    destination: &Path,
    source_kind: Option<formats::Kind>,
    mut warnings: Vec<ReportWarning>,
) -> ExecutionReport {
    let music = snapshot_music(source, destination, source_kind, &mut warnings);
    ExecutionReport {
        summary,
        warnings,
        music: Some(music),
    }
}
impl Drop for StagedDecrypt {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
pub fn preflight(config: &RunConfig, source: &Path) -> Result<Option<PlannedJob>, String> {
    let keys = KeySources::from_config(config);
    let metadata = MetadataClient::new(config.metadata_jobs.get());
    preflight_with_resources(config, &keys, &metadata, source)
}
pub fn preflight_with_keys(
    config: &RunConfig,
    keys: &KeySources,
    source: &Path,
) -> Result<Option<PlannedJob>, String> {
    let metadata = MetadataClient::new(config.metadata_jobs.get());
    preflight_with_resources(config, keys, &metadata, source)
}
pub fn preflight_with_resources(
    config: &RunConfig,
    keys: &KeySources,
    _metadata: &MetadataClient,
    source: &Path,
) -> Result<Option<PlannedJob>, String> {
    match formats::prepare(source).map_err(|e| e.to_string())? {
        PreparedClassification::Passthrough | PreparedClassification::Unknown => {
            if !config.copy_passthrough {
                return Ok(None);
            }
            let destination = plan(config, source, None)
                .map_err(|e| e.to_string())?
                .destination;
            Ok(Some(PlannedJob {
                source: source.into(),
                destination,
                action: PlannedAction::Copy,
            }))
        }
        PreparedClassification::Encrypted(prepared) => {
            let ext = prepared
                .probe_extension(config.qmc_ekey.as_deref(), keys)
                .map_err(|error| error.to_string())?;
            let destination = plan(config, source, Some(&ext))
                .map_err(|e| e.to_string())?
                .destination;
            Ok(Some(PlannedJob {
                source: source.into(),
                destination,
                action: PlannedAction::Decrypt {
                    prepared,
                    extension: ext,
                },
            }))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stage_decrypt(
    config: &RunConfig,
    keys: &KeySources,
    metadata: &MetadataClient,
    source: &Path,
    destination: &Path,
    prepared: formats::adapter::PreparedDecrypt,
    expected_extension: &str,
    source_before: &SourceFingerprint,
    cancellation: &Cancellation,
    timings: &mut JobTimings,
) -> Result<(StagedDecrypt, Vec<ReportWarning>), String> {
    let parent = destination.parent().ok_or("destination has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let ncm_evidence = if config.update_metadata && prepared.kind() == formats::Kind::Ncm {
        Some(formats::ncm_metadata_evidence(source).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let (mut file, mut staged) = create_stage(parent)?;
    let decrypt_started = Instant::now();
    if let Err(error) = prepared.decrypt_to_expected(
        &mut file,
        config.buffer_size.get(),
        config.qmc_ekey.as_deref(),
        keys,
        expected_extension,
    ) {
        drop(file);
        drop(staged);
        return Err(error.to_string());
    }
    if let Err(error) = file.flush() {
        drop(file);
        drop(staged);
        return Err(error.to_string());
    }
    drop(file);
    timings.decrypt += decrypt_started.elapsed();

    if !config.update_metadata {
        return Ok((staged, Vec::new()));
    }
    let mut warnings = Vec::new();
    let resolve_started = Instant::now();
    let resolved = match metadata::resolve_staged(
        &staged.path,
        source,
        prepared.kind(),
        prepared.metadata_hints(),
        ncm_evidence.as_ref(),
        metadata,
    ) {
        Ok(resolved) => resolved,
        Err(error) => {
            timings.metadata_resolve += resolve_started.elapsed();
            tracing::warn!(path=%source.display(), %error, "metadata update skipped");
            warnings.push(ReportWarning {
                code: "metadata_update_skipped".into(),
                detail: error,
            });
            return Ok((staged, warnings));
        }
    };
    timings.metadata_resolve += resolve_started.elapsed();
    let Some(resolved) = resolved else {
        return Ok((staged, warnings));
    };
    let tag_started = Instant::now();
    if let Err(error) = metadata::apply_resolved_in_place(&staged.path, &resolved) {
        timings.tag_write += tag_started.elapsed();
        tracing::warn!(path=%source.display(), %error, "metadata update skipped; regenerating clean audio");
        warnings.push(ReportWarning {
            code: "metadata_update_recovered".into(),
            detail: error,
        });
        let fallback_started = Instant::now();
        let clean_stage = regenerate_clean_stage(
            config,
            keys,
            source,
            parent,
            &prepared,
            expected_extension,
            source_before,
            cancellation,
        )?;
        timings.fallback += fallback_started.elapsed();
        drop(staged);
        staged = clean_stage;
    } else {
        timings.tag_write += tag_started.elapsed();
    }
    Ok((staged, warnings))
}

#[derive(PartialEq, Eq)]
struct SourceFingerprint {
    len: u64,
    modified: Option<SystemTime>,
}

fn source_fingerprint(source: &Path) -> Result<SourceFingerprint, String> {
    let metadata = fs::metadata(source).map_err(|error| error.to_string())?;
    Ok(SourceFingerprint {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn publish_source_is_current(
    source: &Path,
    source_before: &SourceFingerprint,
    cancellation: &Cancellation,
) -> Result<bool, String> {
    if cancellation.is_cancelled() {
        return Ok(false);
    }
    if &source_fingerprint(source)? != source_before {
        return Err("source changed during processing; refusing to publish stale output".into());
    }
    Ok(true)
}

fn create_stage(parent: &Path) -> Result<(fs::File, StagedDecrypt), String> {
    let (file, path) = tempfile::Builder::new()
        .prefix(".unlockm-stage-")
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?
        .keep()
        .map_err(|e| e.error.to_string())?;
    Ok((file, StagedDecrypt { path }))
}

#[allow(clippy::too_many_arguments)]
fn regenerate_clean_stage(
    config: &RunConfig,
    keys: &KeySources,
    source: &Path,
    parent: &Path,
    prepared: &formats::adapter::PreparedDecrypt,
    expected_extension: &str,
    source_before: &SourceFingerprint,
    cancellation: &Cancellation,
) -> Result<StagedDecrypt, String> {
    if cancellation.is_cancelled() {
        return Err("cancelled while recovering from metadata write failure".into());
    }
    if &source_fingerprint(source)? != source_before {
        return Err("source changed while recovering from metadata write failure".into());
    }
    let (mut file, stage) = create_stage(parent)?;
    if let Err(error) = prepared.decrypt_to_expected(
        &mut file,
        config.buffer_size.get(),
        config.qmc_ekey.as_deref(),
        keys,
        expected_extension,
    ) {
        drop(file);
        drop(stage);
        return Err(format!(
            "regenerate clean audio after metadata failure: {error}"
        ));
    }
    if let Err(error) = file.flush() {
        drop(file);
        drop(stage);
        return Err(error.to_string());
    }
    drop(file);
    Ok(stage)
}

fn reserve(
    reservations: &Arc<Mutex<Reservations>>,
    source: &Path,
    destination: &Path,
) -> Result<(), ()> {
    reservations
        .lock()
        .expect("reservation lock")
        .reserve(destination, source)
        .map_err(|_| ())
}
pub fn process(
    config: &RunConfig,
    reservations: &Arc<Mutex<Reservations>>,
    source: &Path,
) -> Summary {
    let result = (|| -> Result<Summary, String> {
        match preflight(config, source)? {
            Some(planned) => execute(config, reservations, planned),
            None => Ok(Summary {
                skipped: 1,
                ..Default::default()
            }),
        }
    })();
    match result {
        Ok(s) => s,
        Err(error) => {
            tracing::warn!(path=%source.display(),%error,"file processing failed");
            if error.starts_with("collision") {
                Summary {
                    conflicted: 1,
                    ..Default::default()
                }
            } else {
                Summary {
                    failed: 1,
                    ..Default::default()
                }
            }
        }
    }
}

pub fn execute(
    config: &RunConfig,
    reservations: &Arc<Mutex<Reservations>>,
    planned: PlannedJob,
) -> Result<Summary, String> {
    let keys = KeySources::from_config(config);
    let metadata = MetadataClient::new(config.metadata_jobs.get());
    let mut timings = JobTimings::default();
    execute_cancellable_with_resources(
        config,
        &keys,
        &metadata,
        reservations,
        planned,
        &Cancellation::default(),
        &mut timings,
    )
}

pub fn execute_cancellable(
    config: &RunConfig,
    reservations: &Arc<Mutex<Reservations>>,
    planned: PlannedJob,
    cancellation: &Cancellation,
) -> Result<Summary, String> {
    let keys = KeySources::from_config(config);
    let metadata = MetadataClient::new(config.metadata_jobs.get());
    let mut timings = JobTimings::default();
    execute_cancellable_with_resources(
        config,
        &keys,
        &metadata,
        reservations,
        planned,
        cancellation,
        &mut timings,
    )
}

pub fn execute_cancellable_with_resources(
    config: &RunConfig,
    keys: &KeySources,
    metadata: &MetadataClient,
    reservations: &Arc<Mutex<Reservations>>,
    planned: PlannedJob,
    cancellation: &Cancellation,
    timings: &mut JobTimings,
) -> Result<Summary, String> {
    execute_cancellable_with_resources_report(
        config,
        keys,
        metadata,
        reservations,
        planned,
        cancellation,
        timings,
        false,
    )
    .map(|report| report.summary)
}

#[allow(clippy::too_many_arguments)]
pub fn execute_cancellable_with_resources_report(
    config: &RunConfig,
    keys: &KeySources,
    metadata: &MetadataClient,
    reservations: &Arc<Mutex<Reservations>>,
    planned: PlannedJob,
    cancellation: &Cancellation,
    timings: &mut JobTimings,
    capture_music_snapshot: bool,
) -> Result<ExecutionReport, String> {
    if cancellation.is_cancelled() {
        return Ok(ExecutionReport::default());
    }
    let is_music = matches!(&planned.action, PlannedAction::Decrypt { .. });
    let source_kind = match &planned.action {
        PlannedAction::Decrypt { prepared, .. } => Some(prepared.kind()),
        PlannedAction::Copy => None,
    };
    let source = planned.source.clone();
    let destination = planned.destination.clone();
    if destination == source {
        let summary = Summary {
            skipped: 1,
            ..Default::default()
        };
        if is_music && capture_music_snapshot {
            return Ok(music_execution_report(
                summary,
                &source,
                &destination,
                source_kind,
                Vec::new(),
            ));
        }
        return Ok(ExecutionReport {
            summary,
            ..Default::default()
        });
    }
    reserve(reservations, &planned.source, &planned.destination)
        .map_err(|_| format!("collision at {}", planned.destination.display()))?;
    if !config.overwrite && destination.exists() {
        let summary = Summary {
            skipped: 1,
            ..Default::default()
        };
        if is_music && capture_music_snapshot {
            return Ok(music_execution_report(
                summary,
                &source,
                &destination,
                source_kind,
                Vec::new(),
            ));
        }
        return Ok(ExecutionReport {
            summary,
            ..Default::default()
        });
    }
    match planned.action {
        PlannedAction::Copy => copy_atomic(
            &planned.source,
            &planned.destination,
            config.overwrite,
            cancellation,
            timings,
        )
        .map(|summary| ExecutionReport {
            summary,
            ..Default::default()
        }),
        PlannedAction::Decrypt {
            prepared,
            extension,
        } => {
            if cancellation.is_cancelled() {
                return Ok(ExecutionReport::default());
            }
            let source_before = source_fingerprint(&planned.source)?;
            let (staged, warnings) = stage_decrypt(
                config,
                keys,
                metadata,
                &planned.source,
                &planned.destination,
                prepared,
                &extension,
                &source_before,
                cancellation,
                timings,
            )?;
            if !publish_source_is_current(&planned.source, &source_before, cancellation)? {
                return Ok(ExecutionReport {
                    summary: Summary::default(),
                    warnings,
                    ..Default::default()
                });
            }
            let publish_started = Instant::now();
            let result = AtomicOutput::publish_existing_temp(
                staged.path.clone(),
                &planned.destination,
                config.overwrite,
            )
            .map_err(|e| e.to_string())?;
            timings.publish += publish_started.elapsed();
            if result && let Ok(metadata) = fs::metadata(&planned.source) {
                let _ = fs::set_permissions(&planned.destination, metadata.permissions());
                let _ = filetime::set_file_mtime(
                    &planned.destination,
                    filetime::FileTime::from_last_modification_time(&metadata),
                );
            }
            let summary = Summary {
                converted: usize::from(result),
                skipped: usize::from(!result),
                ..Default::default()
            };
            if capture_music_snapshot {
                Ok(music_execution_report(
                    summary,
                    &source,
                    &destination,
                    source_kind,
                    warnings,
                ))
            } else {
                Ok(ExecutionReport {
                    summary,
                    warnings,
                    ..Default::default()
                })
            }
        }
    }
}
fn copy_atomic(
    source: &Path,
    destination: &Path,
    overwrite: bool,
    cancellation: &Cancellation,
    timings: &mut JobTimings,
) -> Result<Summary, String> {
    let source_before = source_fingerprint(source)?;
    let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
    let mut output = AtomicOutput::create(destination, overwrite).map_err(|e| e.to_string())?;
    let copy_started = Instant::now();
    std::io::copy(&mut input, output.writer()).map_err(|e| e.to_string())?;
    timings.copy += copy_started.elapsed();
    if cancellation.is_cancelled() {
        output.abort();
        return Ok(Summary::default());
    }
    if !publish_source_is_current(source, &source_before, cancellation)? {
        output.abort();
        return Ok(Summary::default());
    }
    let publish_started = Instant::now();
    let published = output.publish().map_err(|e| e.to_string())?;
    timings.publish += publish_started.elapsed();
    if published {
        if let Ok(metadata) = fs::metadata(source) {
            let _ = fs::set_permissions(destination, metadata.permissions());
            let _ = filetime::set_file_mtime(
                destination,
                filetime::FileTime::from_last_modification_time(&metadata),
            );
        }
        Ok(Summary {
            copied: 1,
            ..Default::default()
        })
    } else {
        Ok(Summary {
            skipped: 1,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};

    fn qmc_recovery_fixture() -> (
        tempfile::TempDir,
        RunConfig,
        std::path::PathBuf,
        formats::adapter::PreparedDecrypt,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("recovery.qmc0");
        let mut encrypted = b"fLaC".to_vec();
        encrypted.resize(4096, 0);
        crate::crypto::qmc::v1::decrypt(&mut encrypted, 0);
        fs::write(&source, encrypted).unwrap();
        let config = RunConfig::new(
            source.clone(),
            Some(directory.path().join("out")),
            false,
            false,
            false,
            false,
            false,
            1,
            64,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let prepared = match formats::prepare(&config.input).unwrap() {
            PreparedClassification::Encrypted(prepared) => prepared,
            _ => panic!("fixture must be encrypted"),
        };
        let canonical_source = config.input.clone();
        (directory, config, canonical_source, prepared)
    }

    #[test]
    fn cancellation_never_replaces_an_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.lrc");
        let destination = directory.path().join("destination.lrc");
        fs::write(&source, vec![b'x'; 2 * 1024 * 1024]).unwrap();
        fs::write(&destination, b"old").unwrap();
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let outcome = copy_atomic(
            &source,
            &destination,
            true,
            &cancellation,
            &mut JobTimings::default(),
        )
        .unwrap();
        assert_eq!(outcome.copied + outcome.skipped, 0);
        assert_eq!(fs::read(&destination).unwrap(), b"old");
        assert_eq!(
            fs::read_dir(directory.path())
                .unwrap()
                .filter_map(std::result::Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(".unlockm-"))
                .count(),
            0
        );
    }

    #[test]
    fn clean_stage_regeneration_is_bounded_by_source_snapshot_and_cancellation() {
        let (directory, config, source, prepared) = qmc_recovery_fixture();
        let parent = directory.path().join("out");
        fs::create_dir(&parent).unwrap();
        let fingerprint = source_fingerprint(&source).unwrap();
        let cancellation = Cancellation::default();
        let stage = regenerate_clean_stage(
            &config,
            &KeySources::default(),
            &source,
            &parent,
            &prepared,
            "flac",
            &fingerprint,
            &cancellation,
        )
        .unwrap();
        assert!(fs::read(&stage.path).unwrap().starts_with(b"fLaC"));
        drop(stage);

        let mut changed = fs::read(&source).unwrap();
        changed.push(0);
        fs::write(&source, changed).unwrap();
        assert!(
            regenerate_clean_stage(
                &config,
                &KeySources::default(),
                &source,
                &parent,
                &prepared,
                "flac",
                &fingerprint,
                &cancellation,
            )
            .unwrap_err()
            .contains("source changed")
        );

        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(
            regenerate_clean_stage(
                &config,
                &KeySources::default(),
                &source,
                &parent,
                &prepared,
                "flac",
                &source_fingerprint(&source).unwrap(),
                &cancelled,
            )
            .unwrap_err()
            .contains("cancelled")
        );
    }

    #[test]
    fn staged_decrypt_never_publishes_after_cancel_or_source_change() {
        for change_source in [false, true] {
            let (directory, config, source, prepared) = qmc_recovery_fixture();
            let destination = directory.path().join("out/recovery.flac");
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(&destination, b"old destination").unwrap();
            let fingerprint = source_fingerprint(&source).unwrap();
            let cancellation = Cancellation::default();
            let (stage, warnings) = stage_decrypt(
                &config,
                &KeySources::default(),
                &MetadataClient::new(1),
                &source,
                &destination,
                prepared,
                "flac",
                &fingerprint,
                &cancellation,
                &mut JobTimings::default(),
            )
            .unwrap();
            assert!(warnings.is_empty());
            assert!(stage.path.is_file());

            if change_source {
                let mut changed = fs::read(&source).unwrap();
                changed.push(0);
                fs::write(&source, changed).unwrap();
                assert!(
                    publish_source_is_current(&source, &fingerprint, &cancellation)
                        .unwrap_err()
                        .contains("source changed")
                );
            } else {
                cancellation.cancel();
                assert!(!publish_source_is_current(&source, &fingerprint, &cancellation).unwrap());
            }
            drop(stage);
            assert_eq!(fs::read(&destination).unwrap(), b"old destination");
            assert!(
                fs::read_dir(destination.parent().unwrap())
                    .unwrap()
                    .filter_map(Result::ok)
                    .all(|entry| !entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".unlockm-stage-"))
            );
        }
    }

    #[test]
    fn local_music_snapshot_uses_only_the_published_destination() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("encrypted-track.ncm");
        let destination = directory.path().join("track.mp3");
        std::fs::write(&source, b"encrypted source is never parsed for history").unwrap();
        let bytes = STANDARD
            .decode(include_str!("../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        std::fs::write(&destination, bytes).unwrap();

        let mut warnings = Vec::new();
        let snapshot = snapshot_music(&source, &destination, None, &mut warnings);
        assert_eq!(snapshot.title.as_deref(), Some("encrypted-track"));
        assert!(snapshot.duration_ms.is_some());
        assert!(warnings.is_empty());
    }

    #[test]
    fn filename_fallback_extracts_conservative_artist_and_title() {
        let snapshot = fallback_music_snapshot(Path::new("Additive Cookie - Token Song.ncm"));
        assert_eq!(snapshot.artist.as_deref(), Some("Additive Cookie"));
        assert_eq!(snapshot.title.as_deref(), Some("Token Song"));
    }

    #[test]
    fn metadata_off_history_snapshot_uses_local_ncm_artist_without_mutating_audio() {
        let directory = tempfile::tempdir().unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../samples/Claire Rosinkranz - Frankenstein.ncm");
        let destination = directory.path().join("output.mp3");
        let audio = STANDARD
            .decode(include_str!("../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        fs::write(&destination, &audio).unwrap();
        let expected = crate::metadata::ncm::parse(&source).unwrap();
        let expected_title = expected
            .title
            .map(|value| value.text)
            .unwrap_or_else(|| "Frankenstein".into());
        let before = fs::read(&destination).unwrap();

        let mut warnings = Vec::new();
        let snapshot = snapshot_music(
            &source,
            &destination,
            Some(formats::Kind::Ncm),
            &mut warnings,
        );

        assert_eq!(snapshot.artist.as_deref(), Some("Claire Rosinkranz"));
        assert_eq!(snapshot.title.as_deref(), Some(expected_title.as_str()));
        assert_eq!(fs::read(destination).unwrap(), before);
        assert!(warnings.is_empty());
    }

    #[test]
    fn artwork_mime_detection_accepts_supported_local_images() {
        assert_eq!(
            artwork_mime_type(b"\x89PNG\r\n\x1a\nbody"),
            Some("image/png")
        );
        assert_eq!(artwork_mime_type(b"\xff\xd8\xffbody"), Some("image/jpeg"));
        assert_eq!(artwork_mime_type(b"not an image"), None);
    }

    #[test]
    fn output_facts_use_final_bytes_and_content_probe() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("extensionless-output");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(include_str!("../../tests/fixtures/lofty-silent.mp3.b64").trim())
            .unwrap();
        fs::write(&output, &bytes).unwrap();

        assert_eq!(
            output_facts(&output).unwrap(),
            (bytes.len() as u64, Some("mp3".into()))
        );
    }

    #[test]
    fn execution_reads_final_tags_only_when_snapshot_capability_is_requested() {
        let (directory, config, source, prepared) = qmc_recovery_fixture();
        let destination = directory.path().join("out/recovery.flac");
        let mut timings = JobTimings::default();
        let report = execute_cancellable_with_resources_report(
            &config,
            &KeySources::default(),
            &MetadataClient::new(1),
            &Arc::new(Mutex::new(Reservations::default())),
            PlannedJob {
                source,
                destination: destination.clone(),
                action: PlannedAction::Decrypt {
                    prepared,
                    extension: "flac".into(),
                },
            },
            &Cancellation::default(),
            &mut timings,
            false,
        )
        .unwrap();
        assert_eq!(report.summary.converted, 1);
        assert!(report.music.is_none());
        assert!(report.warnings.is_empty());
        assert!(destination.is_file());
    }
}
