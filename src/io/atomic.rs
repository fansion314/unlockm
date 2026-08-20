use crate::error::{AppError, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const TEMP_PREFIX: &str = ".unlockm-";
pub fn is_owned_temp(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(TEMP_PREFIX))
}

pub struct AtomicOutput {
    temp: PathBuf,
    final_path: PathBuf,
    file: Option<File>,
    overwrite: bool,
}
impl AtomicOutput {
    pub fn create(destination: &Path, overwrite: bool) -> Result<Self> {
        let parent = destination
            .parent()
            .ok_or_else(|| AppError::Config("destination has no parent".into()))?;
        fs::create_dir_all(parent).map_err(|source| AppError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let temp = tempfile::Builder::new()
            .prefix(TEMP_PREFIX)
            .tempfile_in(parent)
            .map_err(|source| AppError::Io {
                path: parent.to_path_buf(),
                source,
            })?
            .into_temp_path()
            .keep()
            .map_err(|e| AppError::Io {
                path: destination.to_path_buf(),
                source: e.error,
            })?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(true)
            .open(&temp)
            .map_err(|source| AppError::Io {
                path: temp.clone(),
                source,
            })?;
        Ok(Self {
            temp,
            final_path: destination.to_path_buf(),
            file: Some(file),
            overwrite,
        })
    }
    pub fn writer(&mut self) -> &mut File {
        self.file.as_mut().expect("writer exists before publish")
    }
    pub fn publish(mut self) -> Result<bool> {
        self.file
            .take()
            .expect("writer exists before publish")
            .flush()
            .map_err(|source| AppError::Io {
                path: self.temp.clone(),
                source,
            })?;
        publish_temp(self.temp.clone(), &self.final_path, self.overwrite)
    }
    /// Publish an already-complete, same-filesystem owned temp. This is used
    /// for staged decrypts so the final pipeline does not copy a full audio
    /// payload a second time solely to obtain atomic publication.
    pub fn publish_existing_temp(
        temp: PathBuf,
        destination: &Path,
        overwrite: bool,
    ) -> Result<bool> {
        publish_temp(temp, destination, overwrite)
    }
    /// Testable failure/cancellation path: removes only the owned temporary.
    pub fn abort(mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.temp);
    }
}

fn publish_temp(temp: PathBuf, destination: &Path, overwrite: bool) -> Result<bool> {
    publish_temp_with(temp, destination, overwrite, |from, to| {
        fs::rename(from, to)
    })
}

fn publish_temp_with<F>(
    temp: PathBuf,
    destination: &Path,
    overwrite: bool,
    mut rename: F,
) -> Result<bool>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    if destination.exists() && !overwrite {
        let _ = fs::remove_file(&temp);
        return Ok(false);
    }
    // `rename` replaces atomically on Unix but not on Windows. Never
    // delete an old destination before the complete temp is ready: move it
    // aside, publish, then remove the backup. If publish fails, restore it.
    let backup = if overwrite && destination.exists() {
        let parent = destination.parent().expect("validated on create");
        let backup = tempfile::Builder::new()
            .prefix(".unlockm-backup-")
            .tempfile_in(parent)
            .map_err(|source| AppError::Io {
                path: parent.to_path_buf(),
                source,
            })?
            .into_temp_path()
            .keep()
            .map_err(|e| AppError::Io {
                path: destination.to_path_buf(),
                source: e.error,
            })?;
        fs::remove_file(&backup).map_err(|source| AppError::Io {
            path: backup.clone(),
            source,
        })?;
        rename(destination, &backup).map_err(|source| AppError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
        Some(backup)
    } else {
        None
    };
    if let Err(source) = rename(&temp, destination) {
        if let Some(backup) = &backup {
            let _ = rename(backup, destination);
        }
        let _ = fs::remove_file(&temp);
        return Err(AppError::Io {
            path: destination.to_path_buf(),
            source,
        });
    }
    if let Some(backup) = backup {
        // The new output is already durable. A transient cleanup failure
        // must not report this job as failed or invite a second overwrite;
        // owned backup names are ignored by scans and can be reclaimed.
        let _ = fs::remove_file(&backup);
    }
    Ok(true)
}
impl Drop for AtomicOutput {
    fn drop(&mut self) {
        if self.file.is_some() {
            let _ = fs::remove_file(&self.temp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    #[test]
    fn skip_preserves_existing_output() {
        let d = tempdir().unwrap();
        let p = d.path().join("out");
        fs::write(&p, b"old").unwrap();
        let mut out = AtomicOutput::create(&p, false).unwrap();
        out.writer().write_all(b"new").unwrap();
        assert!(!out.publish().unwrap());
        assert_eq!(fs::read(&p).unwrap(), b"old");
    }
    #[test]
    fn dropped_output_is_not_published() {
        let d = tempdir().unwrap();
        let p = d.path().join("out");
        {
            let mut out = AtomicOutput::create(&p, false).unwrap();
            out.writer().write_all(b"new").unwrap();
        }
        assert!(!p.exists());
    }
    #[test]
    fn overwrite_replaces_only_after_complete_temp() {
        let d = tempdir().unwrap();
        let p = d.path().join("out");
        fs::write(&p, b"old").unwrap();
        let mut out = AtomicOutput::create(&p, true).unwrap();
        out.writer().write_all(b"new").unwrap();
        assert!(out.publish().unwrap());
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert_eq!(
            fs::read_dir(d.path())
                .unwrap()
                .filter_map(std::result::Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".unlockm-backup-"))
                .count(),
            0
        );
    }
    #[test]
    fn injected_abort_preserves_existing_and_cleans_temp() {
        let d = tempdir().unwrap();
        let p = d.path().join("out");
        fs::write(&p, b"old").unwrap();
        let mut out = AtomicOutput::create(&p, true).unwrap();
        out.writer().write_all(b"new").unwrap();
        out.abort();
        assert_eq!(fs::read(&p).unwrap(), b"old");
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 1);
    }
    #[test]
    fn failed_final_rename_restores_destination_and_cleans_owned_artifacts() {
        let d = tempdir().unwrap();
        let destination = d.path().join("out");
        let temp = d.path().join(".unlockm-stage-test");
        fs::write(&destination, b"old").unwrap();
        fs::write(&temp, b"new").unwrap();
        let result = publish_temp_with(temp.clone(), &destination, true, |from, to| {
            if from == temp && to == destination {
                return Err(std::io::Error::other("injected final rename failure"));
            }
            fs::rename(from, to)
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"old");
        assert!(!temp.exists());
        assert_eq!(
            fs::read_dir(d.path())
                .unwrap()
                .filter_map(std::result::Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".unlockm-backup-"))
                .count(),
            0
        );
    }
}
