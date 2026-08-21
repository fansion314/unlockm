use crate::{
    error::{AppError, Result},
    presentation::ProgressReporter,
};
use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputLayout {
    Mirror,
    Flat,
}

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub input: PathBuf,
    pub input_is_dir: bool,
    pub input_base: PathBuf,
    pub output: PathBuf,
    pub layout: OutputLayout,
    pub copy_passthrough: bool,
    pub overwrite: bool,
    pub watch: bool,
    pub update_metadata: bool,
    pub jobs: NonZeroUsize,
    pub buffer_size: NonZeroUsize,
    pub metadata_jobs: NonZeroUsize,
    pub qmc_mmkv: Option<PathBuf>,
    pub qmc_mmkv_key: Option<String>,
    pub qmc_key_map: Option<PathBuf>,
    pub qmc_ekey: Option<String>,
    pub kgg_db: Option<PathBuf>,
    pub presentation: ProgressReporter,
}

impl RunConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input: PathBuf,
        output: Option<PathBuf>,
        flat: bool,
        no_copy: bool,
        overwrite: bool,
        watch: bool,
        update_metadata: bool,
        jobs: usize,
        buffer_size: usize,
        metadata_jobs: usize,
        qmc_mmkv: Option<PathBuf>,
        qmc_mmkv_key: Option<String>,
        qmc_key_map: Option<PathBuf>,
        qmc_ekey: Option<String>,
        kgg_db: Option<PathBuf>,
    ) -> Result<Self> {
        let metadata = std::fs::metadata(&input).map_err(|source| AppError::Io {
            path: input.clone(),
            source,
        })?;
        let input_is_dir = metadata.is_dir();
        if !input_is_dir && !metadata.is_file() {
            return Err(AppError::Config(
                "input must be a regular file or directory".into(),
            ));
        }
        if watch && !input_is_dir {
            return Err(AppError::Config(
                "--watch requires a directory input".into(),
            ));
        }
        let input = input.canonicalize().map_err(|source| AppError::Io {
            path: input.clone(),
            source,
        })?;
        let input_base = if input_is_dir {
            input.clone()
        } else {
            input
                .parent()
                .ok_or_else(|| AppError::Config("file input has no parent".into()))?
                .to_path_buf()
        };
        let output = normalize_output(output.unwrap_or_else(|| input_base.clone()))?;
        let jobs = NonZeroUsize::new(jobs)
            .ok_or_else(|| AppError::Config("--jobs must be greater than zero".into()))?;
        let buffer_size = NonZeroUsize::new(buffer_size)
            .ok_or_else(|| AppError::Config("--buffer-size must be greater than zero".into()))?;
        let metadata_jobs = NonZeroUsize::new(metadata_jobs)
            .ok_or_else(|| AppError::Config("--metadata-jobs must be greater than zero".into()))?;
        Ok(Self {
            input,
            input_is_dir,
            input_base,
            output,
            layout: if flat {
                OutputLayout::Flat
            } else {
                OutputLayout::Mirror
            },
            copy_passthrough: !no_copy,
            overwrite,
            watch,
            update_metadata,
            jobs,
            buffer_size,
            metadata_jobs,
            qmc_mmkv,
            qmc_mmkv_key,
            qmc_key_map,
            qmc_ekey,
            kgg_db,
            presentation: ProgressReporter::disabled(),
        })
    }
    pub fn output_is_proper_child(&self) -> bool {
        self.input_is_dir && self.output.starts_with(&self.input) && self.output != self.input
    }
    pub fn relative_input(&self, path: &Path) -> Result<PathBuf> {
        path.strip_prefix(&self.input_base)
            .map(Path::to_path_buf)
            .map_err(|_| AppError::Config(format!("{} is outside input root", path.display())))
    }
}

fn normalize_output(output: PathBuf) -> Result<PathBuf> {
    if output.exists() {
        return output.canonicalize().map_err(|source| AppError::Io {
            path: output,
            source,
        });
    }
    let parent = output.parent().unwrap_or(Path::new("."));
    let parent = parent.canonicalize().map_err(|source| AppError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    Ok(parent.join(
        output
            .file_name()
            .ok_or_else(|| AppError::Config("output has no filename".into()))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn watch_rejects_a_file() {
        let temp = tempdir().unwrap();
        let input = temp.path().join("track.ncm");
        std::fs::write(&input, b"x").unwrap();
        assert!(
            RunConfig::new(
                input, None, false, false, false, true, false, 1, 1, 1, None, None, None, None,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn zero_limits_are_rejected() {
        let temp = tempdir().unwrap();
        assert!(
            RunConfig::new(
                temp.path().to_path_buf(),
                None,
                false,
                false,
                false,
                false,
                false,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
                None
            )
            .is_err()
        );
    }
}
