//! Key acquisition stays outside format adapters.  The values kept here are
//! encrypted ekeys, never decoded/raw cipher keys, and diagnostics only name
//! identifiers or sources.
pub mod kgm;
pub mod qmc;

use crate::config::RunConfig;
use std::{collections::HashMap, path::Path};

#[derive(Debug, Default)]
pub struct KeySources {
    qmc: HashMap<String, String>,
    kgm: HashMap<String, String>,
}

impl KeySources {
    pub fn with_kgm_ekeys(kgm: HashMap<String, String>) -> Self {
        Self {
            qmc: HashMap::new(),
            kgm,
        }
    }
    pub fn from_config(config: &RunConfig) -> Self {
        let mut result = Self::default();
        if let Some(path) = &config.qmc_key_map {
            match qmc::load_key_map(path) {
                Ok(map) => result.qmc.extend(map),
                Err(error) => {
                    tracing::warn!(source=%path.display(), %error, "could not load QMC key map")
                }
            }
        }
        if let Some(path) = &config.qmc_mmkv {
            match qmc::load_mmkv(path, config.qmc_mmkv_key.as_deref()) {
                Ok(map) => result.qmc.extend(map),
                Err(error) => {
                    tracing::warn!(source=%path.display(), %error, "could not load QMC MMKV")
                }
            }
        }
        result.qmc.extend(qmc::discover_macos());
        if let Some(path) = &config.kgg_db {
            match kgm::load_ekeys(path) {
                Ok(map) => result.kgm = map,
                Err(error) => {
                    tracing::warn!(source=%path.display(), %error, "could not load KGM key database")
                }
            }
        } else if let Some(path) = kgm::windows_default_database() {
            match kgm::load_ekeys(&path) {
                Ok(map) => result.kgm = map,
                Err(error) => {
                    tracing::warn!(source=%path.display(), %error, "could not load discovered KGM key database")
                }
            }
        }
        result
    }

    pub fn qmc_ekey(&self, path: &Path, footer_filename: Option<&str>) -> Option<&str> {
        qmc::lookup(&self.qmc, path, footer_filename)
    }

    pub fn kgm_ekey(&self, audio_hash: &str) -> Option<&str> {
        self.kgm.get(audio_hash).map(String::as_str)
    }
}
