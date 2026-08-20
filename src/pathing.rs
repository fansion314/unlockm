use crate::{
    config::{OutputLayout, RunConfig},
    error::{AppError, Result},
};
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedPath {
    pub relative: PathBuf,
    pub destination: PathBuf,
}

pub fn plan(
    config: &RunConfig,
    source: &Path,
    replacement_extension: Option<&str>,
) -> Result<PlannedPath> {
    let relative = config.relative_input(source)?;
    let filename = relative
        .file_name()
        .ok_or_else(|| AppError::Config("input path has no filename".into()))?;
    let mut leaf = PathBuf::from(filename);
    if let Some(extension) = replacement_extension {
        let filename_text = filename.to_string_lossy();
        let stem = encrypted_stem(&filename_text);
        leaf = PathBuf::from(format!("{stem}.{extension}"));
    }
    let destination = match config.layout {
        OutputLayout::Mirror => config
            .output
            .join(relative.parent().unwrap_or(Path::new("")))
            .join(leaf),
        OutputLayout::Flat => config.output.join(leaf),
    };
    ensure_contained(&config.output, &destination)?;
    Ok(PlannedPath {
        relative,
        destination,
    })
}

/// Remove one encrypted suffix, including recognised multi-dot variants.
pub fn encrypted_stem(name: &str) -> &str {
    const MULTI: &[&str] = &[".kgm.flac", ".vpr.flac"];
    for suffix in MULTI {
        if name.to_ascii_lowercase().ends_with(suffix) {
            return &name[..name.len() - suffix.len()];
        }
    }
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

pub fn ensure_contained(root: &Path, child: &Path) -> Result<()> {
    if child
        .components()
        .any(|c| matches!(c, Component::ParentDir))
        || !child.starts_with(root)
    {
        return Err(AppError::Config(format!(
            "output escapes configured root: {}",
            child.display()
        )));
    }
    Ok(())
}

#[derive(Default, Debug)]
pub struct Reservations {
    claims: HashMap<PathBuf, PathBuf>,
}
impl Reservations {
    pub fn reserve(&mut self, destination: &Path, source: &Path) -> Result<()> {
        let key = collision_key(destination);
        if let Some(first) = self.claims.get(&key) {
            if first != source {
                return Err(AppError::Collision(destination.to_path_buf()));
            }
        } else {
            self.claims.insert(key, source.to_path_buf());
        }
        Ok(())
    }
}
pub fn collision_key(path: &Path) -> PathBuf {
    #[cfg(any(windows, target_os = "macos"))]
    {
        PathBuf::from(path.to_string_lossy().to_ascii_lowercase())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RunConfig;
    use tempfile::tempdir;
    fn config(flat: bool) -> (tempfile::TempDir, RunConfig) {
        let temp = tempdir().unwrap();
        let input = temp.path().join("input");
        let output = temp.path().join("out");
        std::fs::create_dir(&input).unwrap();
        let c = RunConfig::new(
            input,
            Some(output),
            flat,
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
        (temp, c)
    }
    #[test]
    fn mirror_keeps_parents() {
        let (_t, c) = config(false);
        let p = c.input.join("a/b/song.ncm");
        let result = plan(&c, &p, Some("flac")).unwrap();
        assert_eq!(result.destination, c.output.join("a/b/song.flac"));
    }
    #[test]
    fn flat_drops_parents() {
        let (_t, c) = config(true);
        let result = plan(&c, &c.input.join("a/b/song.ncm"), Some("mp3")).unwrap();
        assert_eq!(result.destination, c.output.join("song.mp3"));
    }
    #[test]
    fn multidot_stem() {
        assert_eq!(encrypted_stem("x.kgm.flac"), "x");
    }
    #[test]
    fn collision_is_rejected() {
        let mut r = Reservations::default();
        r.reserve(Path::new("x"), Path::new("a")).unwrap();
        assert!(r.reserve(Path::new("x"), Path::new("b")).is_err());
    }
    #[test]
    fn collision_key_is_platform_aware() {
        let a = collision_key(Path::new("Song.FLAC"));
        let b = collision_key(Path::new("song.flac"));
        #[cfg(any(windows, target_os = "macos"))]
        assert_eq!(a, b);
        #[cfg(not(any(windows, target_os = "macos")))]
        assert_ne!(a, b);
    }
}
