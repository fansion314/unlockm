use crate::{
    config::RunConfig,
    error::{AppError, Result},
    io::atomic::is_owned_temp,
};
use std::{fs, path::PathBuf};
use walkdir::WalkDir;

pub fn enumerate(config: &RunConfig) -> Result<Vec<PathBuf>> {
    if !config.input_is_dir {
        return Ok(vec![config.input.clone()]);
    }
    let mut paths = Vec::new();
    for entry in WalkDir::new(&config.input)
        .follow_links(false)
        .into_iter()
        .filter_map(|x| x.ok())
    {
        let p = entry.path();
        if p == config.input {
            continue;
        };
        if config.output_is_proper_child() && p.starts_with(&config.output) {
            continue;
        }
        if is_owned_temp(p) {
            continue;
        }
        if entry.file_type().is_symlink() {
            tracing::warn!(path=%p.display(),"skipping symlink");
            continue;
        }
        if entry.file_type().is_file() {
            paths.push(p.to_path_buf());
        }
        if entry.file_type().is_dir()
            && matches!(config.layout, crate::config::OutputLayout::Mirror)
        {
            let rel = config.relative_input(p)?;
            fs::create_dir_all(config.output.join(rel)).map_err(|source| AppError::Io {
                path: p.to_path_buf(),
                source,
            })?;
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RunConfig;
    use tempfile::tempdir;
    #[test]
    fn child_output_is_pruned() {
        let d = tempdir().unwrap();
        let input = d.path().join("i");
        let output = input.join("out");
        fs::create_dir_all(&output).unwrap();
        fs::write(input.join("a.txt"), b"a").unwrap();
        fs::write(output.join("b.txt"), b"b").unwrap();
        let c = RunConfig::new(
            input,
            Some(output),
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
        assert_eq!(enumerate(&c).unwrap().len(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_is_not_enumerated() {
        use std::os::unix::fs::symlink;
        let d = tempdir().unwrap();
        let input = d.path().join("i");
        let output = d.path().join("o");
        std::fs::create_dir(&input).unwrap();
        let outside = d.path().join("outside.lrc");
        std::fs::write(&outside, b"x").unwrap();
        symlink(&outside, input.join("link.lrc")).unwrap();
        let c = RunConfig::new(
            input,
            Some(output),
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
        assert!(enumerate(&c).unwrap().is_empty());
    }
}
