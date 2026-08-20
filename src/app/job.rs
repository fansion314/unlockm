use super::Summary;
use crate::app::runner::Cancellation;
use crate::{
    config::RunConfig,
    formats::{self, PreparedClassification},
    io::atomic::AtomicOutput,
    keys::KeySources,
    metadata::{self, provider::qq::MetadataClient},
    pathing::{plan, Reservations},
};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub enum PlannedAction {
    Copy,
    StagedDecrypt(StagedDecrypt),
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
    metadata: &MetadataClient,
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
            // A suffix cannot safely reserve the eventual destination: several
            // formats carry an arbitrary native container.  Decrypt once into a
            // private owned temp, sniff its actual extension, and only then make
            // the destination claim available to the batch planner.
            let (staged, ext) = stage_decrypt(config, keys, metadata, source, prepared)?;
            let destination = plan(config, source, Some(&ext))
                .map_err(|e| e.to_string())?
                .destination;
            Ok(Some(PlannedJob {
                source: source.into(),
                destination,
                action: PlannedAction::StagedDecrypt(staged),
            }))
        }
    }
}

fn stage_decrypt(
    config: &RunConfig,
    keys: &KeySources,
    metadata: &MetadataClient,
    source: &Path,
    prepared: formats::adapter::PreparedDecrypt,
) -> Result<(StagedDecrypt, String), String> {
    fs::create_dir_all(&config.output).map_err(|e| e.to_string())?;
    let (mut file, path) = tempfile::Builder::new()
        .prefix(".unlockm-stage-")
        .tempfile_in(&config.output)
        .map_err(|e| e.to_string())?
        .keep()
        .map_err(|e| e.error.to_string())?;
    let staged = StagedDecrypt { path };
    let kind = prepared.kind();
    match prepared.decrypt_to(
        &mut file,
        config.buffer_size.get(),
        config.qmc_ekey.as_deref(),
        keys,
    ) {
        Ok(extension) => {
            file.flush().map_err(|e| e.to_string())?;
            drop(file);
            if config.update_metadata {
                if let Err(error) = metadata::enrich_staged(
                    &staged.path,
                    source,
                    kind,
                    prepared.metadata_hints(),
                    metadata,
                ) {
                    tracing::warn!(path=%source.display(), %error, "metadata update skipped");
                }
            }
            Ok((staged, extension))
        }
        Err(error) => {
            drop(staged);
            Err(error.to_string())
        }
    }
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
    execute_cancellable(config, reservations, planned, &Cancellation::default())
}

pub fn execute_cancellable(
    config: &RunConfig,
    reservations: &Arc<Mutex<Reservations>>,
    planned: PlannedJob,
    cancellation: &Cancellation,
) -> Result<Summary, String> {
    if cancellation.is_cancelled() {
        return Ok(Summary::default());
    }
    if planned.destination == planned.source {
        return Ok(Summary {
            skipped: 1,
            ..Default::default()
        });
    }
    reserve(reservations, &planned.source, &planned.destination)
        .map_err(|_| format!("collision at {}", planned.destination.display()))?;
    match planned.action {
        PlannedAction::Copy => copy_atomic(
            &planned.source,
            &planned.destination,
            config.overwrite,
            cancellation,
        ),
        PlannedAction::StagedDecrypt(staged) => {
            if cancellation.is_cancelled() {
                return Ok(Summary::default());
            }
            let result = AtomicOutput::publish_existing_temp(
                staged.path.clone(),
                &planned.destination,
                config.overwrite,
            )
            .map_err(|e| e.to_string())?;
            if result {
                if let Ok(metadata) = fs::metadata(&planned.source) {
                    let _ = fs::set_permissions(&planned.destination, metadata.permissions());
                    let _ = filetime::set_file_mtime(
                        &planned.destination,
                        filetime::FileTime::from_last_modification_time(&metadata),
                    );
                }
            }
            Ok(Summary {
                converted: usize::from(result),
                skipped: usize::from(!result),
                ..Default::default()
            })
        }
    }
}
fn copy_atomic(
    source: &Path,
    destination: &Path,
    overwrite: bool,
    cancellation: &Cancellation,
) -> Result<Summary, String> {
    let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
    let mut output = AtomicOutput::create(destination, overwrite).map_err(|e| e.to_string())?;
    let mut b = vec![0; 1024 * 1024];
    loop {
        if cancellation.is_cancelled() {
            output.abort();
            return Ok(Summary::default());
        }
        let n = input.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        output
            .writer()
            .write_all(&b[..n])
            .map_err(|e| e.to_string())?;
    }
    if cancellation.is_cancelled() {
        output.abort();
        return Ok(Summary::default());
    }
    if output.publish().map_err(|e| e.to_string())? {
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

    #[test]
    fn cancellation_never_replaces_an_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.lrc");
        let destination = directory.path().join("destination.lrc");
        fs::write(&source, vec![b'x'; 2 * 1024 * 1024]).unwrap();
        fs::write(&destination, b"old").unwrap();
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let outcome = copy_atomic(&source, &destination, true, &cancellation).unwrap();
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
}
