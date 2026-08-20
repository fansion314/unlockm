use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("I/O while processing {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported or corrupt encrypted file {0}")]
    InvalidEncrypted(PathBuf),
    #[error("output collision at {0}")]
    Collision(PathBuf),
    #[error("watch mode failed: {0}")]
    Watch(String),
    #[error("internal task failed: {0}")]
    Internal(String),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        1
    }
}

/// Do not emit credential-like command values in structured diagnostics.
pub fn redact_secret(value: &str) -> String {
    if value.is_empty() {
        "<empty>".into()
    } else {
        format!("<redacted:{} bytes>", value.len())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redact_never_echoes_secret() {
        let secret = "abc123";
        let result = redact_secret(secret);
        assert!(!result.contains(secret));
        assert!(result.contains('6'));
    }
}
