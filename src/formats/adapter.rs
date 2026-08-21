use super::{Kind, decrypt_to, decrypt_to_expected, probe_extension};
use crate::{error::Result, keys::KeySources};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

/// Identity evidence carried by QMC footers. It stays separate from decrypted
/// tags so a provider lookup is always tied to container evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataHints {
    pub resource_id: Option<String>,
    pub media_mid: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NcmMetadataEvidence {
    pub json: Vec<u8>,
    pub image1: Option<Vec<u8>>,
}

/// A registered encrypted format. Implementations own suffix selection and a
/// cheap positive header validation; they never choose destinations or publish
/// files.
pub trait FormatAdapter: Sync {
    fn kind(&self) -> Kind;
    fn suffixes(&self) -> &'static [&'static str];
    fn validates(&self, path: &Path, header: &[u8]) -> bool;

    fn prepare(&self, path: &Path) -> PreparedDecrypt {
        PreparedDecrypt {
            source: path.to_path_buf(),
            kind: self.kind(),
            metadata_hints: MetadataHints::default(),
        }
    }
}

/// An adapter-selected, still-streaming operation. The job layer owns the
/// temporary output and asks this value to write into it exactly once.
#[derive(Debug, Clone)]
pub struct PreparedDecrypt {
    source: PathBuf,
    kind: Kind,
    metadata_hints: MetadataHints,
}

impl PreparedDecrypt {
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn metadata_hints(&self) -> &MetadataHints {
        &self.metadata_hints
    }
    pub(crate) fn set_metadata_hints(&mut self, metadata_hints: MetadataHints) {
        self.metadata_hints = metadata_hints;
    }
    pub fn decrypt_to(
        &self,
        output: &mut File,
        buffer_size: usize,
        qmc_ekey: Option<&str>,
        keys: &KeySources,
    ) -> Result<String> {
        decrypt_to(self.kind, &self.source, output, buffer_size, qmc_ekey, keys)
    }
    pub(crate) fn probe_extension(
        &self,
        qmc_ekey: Option<&str>,
        keys: &KeySources,
    ) -> Result<String> {
        probe_extension(self.kind, &self.source, qmc_ekey, keys)
    }
    pub(crate) fn decrypt_to_expected(
        &self,
        output: &mut File,
        buffer_size: usize,
        qmc_ekey: Option<&str>,
        keys: &KeySources,
        expected_extension: &str,
    ) -> Result<String> {
        decrypt_to_expected(
            self.kind,
            &self.source,
            output,
            buffer_size,
            qmc_ekey,
            keys,
            Some(expected_extension),
        )
    }
}
