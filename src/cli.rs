use crate::{config::RunConfig, error::Result};
use clap::Parser;
use std::{path::PathBuf, sync::OnceLock};
use tracing_subscriber::{fmt, EnvFilter};

fn default_jobs() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
}
#[derive(Debug, Parser)]
#[command(
    name = "unlockm",
    version,
    about = "Convert encrypted music downloads to standard audio"
)]
pub struct Cli {
    #[arg(value_name = "INPUT", required_unless_present = "supported_ext")]
    pub input: Option<PathBuf>,
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub flat: bool,
    #[arg(long)]
    pub no_copy: bool,
    #[arg(long)]
    pub overwrite: bool,
    #[arg(long)]
    pub watch: bool,
    #[arg(long)]
    pub update_metadata: bool,
    #[arg(long, default_value_t = default_jobs())]
    pub jobs: usize,
    #[arg(long, default_value_t = 1024 * 1024)]
    pub buffer_size: usize,
    #[arg(long, default_value_t = 4)]
    pub metadata_jobs: usize,
    #[arg(long)]
    pub qmc_mmkv: Option<PathBuf>,
    #[arg(long)]
    pub qmc_mmkv_key: Option<String>,
    /// JSON object or newline-delimited `identifier=ekey` QMC key map.
    #[arg(long)]
    pub qmc_key_map: Option<PathBuf>,
    /// Override a QMC v2 ekey obtained from the music client.
    #[arg(long)]
    pub qmc_ekey: Option<String>,
    #[arg(long)]
    pub kgg_db: Option<PathBuf>,
    #[arg(long, default_value = "warn")]
    pub log_level: String,
    #[arg(long)]
    pub json_logs: bool,
    #[arg(long)]
    pub supported_ext: bool,
}
impl Cli {
    pub fn into_config(self) -> Result<RunConfig> {
        RunConfig::new(
            self.input
                .expect("clap requires INPUT unless --supported-ext"),
            self.output,
            self.flat,
            self.no_copy,
            self.overwrite,
            self.watch,
            self.update_metadata,
            self.jobs,
            self.buffer_size,
            self.metadata_jobs,
            self.qmc_mmkv,
            self.qmc_mmkv_key,
            self.qmc_key_map,
            self.qmc_ekey,
            self.kgg_db,
        )
    }
    pub fn init_logging(
        &self,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        static INITIALIZED: OnceLock<()> = OnceLock::new();
        if INITIALIZED.set(()).is_ok() {
            let filter = EnvFilter::try_new(format!("unlockm={}", self.log_level))?;
            if self.json_logs {
                fmt().with_env_filter(filter).json().try_init()?;
            } else {
                fmt().with_env_filter(filter).try_init()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_match_the_documented_budgets() {
        let cli = Cli::try_parse_from(["unlockm", "input"]).unwrap();
        assert_eq!(cli.buffer_size, 1024 * 1024);
        assert_eq!(cli.metadata_jobs, 4);
        assert!(!cli.flat && !cli.no_copy && !cli.watch);
    }
    #[test]
    fn supported_ext_does_not_require_input() {
        assert!(Cli::try_parse_from(["unlockm", "--supported-ext"]).is_ok());
    }
}
