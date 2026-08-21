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

/// The automatic concurrency budget used by both the CLI and desktop host.
pub fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(2)
}

pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024;
pub const DEFAULT_METADATA_JOBS: usize = 4;

/// A typed, host-independent request for one `unlockm` batch.
///
/// The CLI converts its parsed flags into this value and the desktop host will
/// build the same value from its narrow request DTO. Keeping defaults here
/// avoids subtle drift between UI and command-line execution.
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub layout: OutputLayout,
    pub copy_passthrough: bool,
    pub overwrite: bool,
    pub watch: bool,
    pub update_metadata: bool,
    pub jobs: usize,
    pub buffer_size: usize,
    pub metadata_jobs: usize,
    pub qmc_mmkv: Option<PathBuf>,
    pub qmc_mmkv_key: Option<String>,
    pub qmc_key_map: Option<PathBuf>,
    pub qmc_ekey: Option<String>,
    pub kgg_db: Option<PathBuf>,
}

impl RunOptions {
    pub fn new(input: PathBuf) -> Self {
        Self {
            input,
            output: None,
            layout: OutputLayout::Mirror,
            copy_passthrough: true,
            overwrite: false,
            watch: false,
            update_metadata: false,
            jobs: default_jobs(),
            buffer_size: DEFAULT_BUFFER_SIZE,
            metadata_jobs: DEFAULT_METADATA_JOBS,
            qmc_mmkv: None,
            qmc_mmkv_key: None,
            qmc_key_map: None,
            qmc_ekey: None,
            kgg_db: None,
        }
    }

    pub fn build(self) -> Result<RunConfig> {
        RunConfig::from_options(self)
    }
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
    pub fn from_options(options: RunOptions) -> Result<Self> {
        let RunOptions {
            input,
            output,
            layout,
            copy_passthrough,
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
        } = options;
        Self::from_parts(
            input,
            output,
            layout,
            copy_passthrough,
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
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        input: PathBuf,
        output: Option<PathBuf>,
        layout: OutputLayout,
        copy_passthrough: bool,
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
            layout,
            copy_passthrough,
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

    /// Compatibility constructor for existing library callers. New hosts use
    /// [`RunOptions`] so named fields and shared defaults are explicit.
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
        Self::from_options(RunOptions {
            input,
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

    #[test]
    fn typed_options_preserve_legacy_constructor_semantics() {
        let temp = tempdir().unwrap();
        let input = temp.path().join("track.ncm");
        std::fs::write(&input, b"x").unwrap();
        let output = temp.path().join("output");

        let legacy = RunConfig::new(
            input.clone(),
            Some(output.clone()),
            true,
            true,
            true,
            false,
            true,
            2,
            4096,
            3,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let typed = RunOptions {
            input,
            output: Some(output),
            layout: OutputLayout::Flat,
            copy_passthrough: false,
            overwrite: true,
            watch: false,
            update_metadata: true,
            jobs: 2,
            buffer_size: 4096,
            metadata_jobs: 3,
            qmc_mmkv: None,
            qmc_mmkv_key: None,
            qmc_key_map: None,
            qmc_ekey: None,
            kgg_db: None,
        }
        .build()
        .unwrap();

        assert_eq!(legacy.input, typed.input);
        assert_eq!(legacy.input_base, typed.input_base);
        assert_eq!(legacy.output, typed.output);
        assert_eq!(legacy.layout, typed.layout);
        assert_eq!(legacy.copy_passthrough, typed.copy_passthrough);
        assert_eq!(legacy.overwrite, typed.overwrite);
        assert_eq!(legacy.watch, typed.watch);
        assert_eq!(legacy.update_metadata, typed.update_metadata);
        assert_eq!(legacy.jobs, typed.jobs);
        assert_eq!(legacy.buffer_size, typed.buffer_size);
        assert_eq!(legacy.metadata_jobs, typed.metadata_jobs);
    }

    #[test]
    fn typed_options_reject_invalid_limits_and_file_watch() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("track.ncm");
        std::fs::write(&file, b"x").unwrap();

        let mut zero_jobs = RunOptions::new(file.clone());
        zero_jobs.jobs = 0;
        assert!(zero_jobs.build().is_err());

        let mut file_watch = RunOptions::new(file);
        file_watch.watch = true;
        assert!(file_watch.build().is_err());
    }
}
