use crate::{
    config::{RunConfig, RunOptions, default_jobs},
    error::{AppError, Result},
    presentation::{ProgressMode, ProgressReporter},
};
use clap::Parser;
use std::{path::PathBuf, sync::OnceLock};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Parser)]
#[command(
    name = "unlockm",
    version,
    about = "Convert encrypted music downloads to standard audio"
)]
pub struct Cli {
    /// File or directory to convert recursively.
    #[arg(value_name = "INPUT", required_unless_present = "supported_ext")]
    pub input: Option<PathBuf>,
    /// Destination directory; defaults to the input file's parent or input directory.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,
    /// Put every output directly in the destination root; same-name claims conflict.
    #[arg(long)]
    pub flat: bool,
    /// Convert encrypted audio without copying ordinary companion files.
    #[arg(long)]
    pub no_copy: bool,
    /// Atomically replace an existing destination after the new file is complete.
    #[arg(long)]
    pub overwrite: bool,
    /// Process the initial directory, then watch recursively for stable changes.
    #[arg(long)]
    pub watch: bool,
    /// Conservatively enrich tags and cover art from verified container/provider data.
    #[arg(long)]
    pub update_metadata: bool,
    /// Maximum concurrent file jobs.
    #[arg(long, default_value_t = default_jobs())]
    pub jobs: usize,
    /// Streaming decrypt buffer bytes per active file.
    #[arg(long, default_value_t = 1024 * 1024)]
    pub buffer_size: usize,
    /// Maximum concurrent metadata network requests.
    #[arg(long, default_value_t = 4)]
    pub metadata_jobs: usize,
    /// Explicit QQMusic MMKV key store path.
    #[arg(long)]
    pub qmc_mmkv: Option<PathBuf>,
    /// Password for an encrypted QQMusic MMKV store.
    #[arg(long)]
    pub qmc_mmkv_key: Option<String>,
    /// JSON object or newline-delimited `identifier=ekey` QMC key map.
    #[arg(long)]
    pub qmc_key_map: Option<PathBuf>,
    /// Override a QMC v2 ekey obtained from the music client.
    #[arg(long)]
    pub qmc_ekey: Option<String>,
    /// KuGou encrypted SQLite key database path for KGM v5.
    #[arg(long)]
    pub kgg_db: Option<PathBuf>,
    /// Tracing filter level: off, error, warn, info, debug, or trace.
    #[arg(long, default_value = "warn")]
    pub log_level: String,
    /// Emit structured JSON diagnostics and a JSON final summary.
    #[arg(long)]
    pub json_logs: bool,
    /// Progress display policy; `auto` renders only on an interactive terminal.
    #[arg(long, value_enum, default_value_t = ProgressMode::Auto)]
    pub progress: ProgressMode,
    /// Print the sorted supported encrypted suffixes and exit.
    #[arg(long)]
    pub supported_ext: bool,
}
impl Cli {
    pub fn progress_reporter(&self) -> ProgressReporter {
        ProgressReporter::new(self.progress, self.json_logs)
    }
    pub fn into_config(self, reporter: ProgressReporter) -> Result<RunConfig> {
        if self.json_logs && self.progress == ProgressMode::Always {
            return Err(AppError::Config(
                "--json-logs conflicts with --progress always".into(),
            ));
        }
        let mut config = RunOptions {
            input: self
                .input
                .expect("clap requires INPUT unless --supported-ext"),
            output: self.output,
            layout: if self.flat {
                crate::config::OutputLayout::Flat
            } else {
                crate::config::OutputLayout::Mirror
            },
            copy_passthrough: !self.no_copy,
            overwrite: self.overwrite,
            watch: self.watch,
            update_metadata: self.update_metadata,
            jobs: self.jobs,
            buffer_size: self.buffer_size,
            metadata_jobs: self.metadata_jobs,
            qmc_mmkv: self.qmc_mmkv,
            qmc_mmkv_key: self.qmc_mmkv_key,
            qmc_key_map: self.qmc_key_map,
            qmc_ekey: self.qmc_ekey,
            kgg_db: self.kgg_db,
        }
        .build()?;
        config.presentation = reporter;
        Ok(config)
    }
    pub fn init_logging(
        &self,
        reporter: ProgressReporter,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        static INITIALIZED: OnceLock<()> = OnceLock::new();
        if INITIALIZED.set(()).is_ok() {
            let filter = EnvFilter::try_new(format!("unlockm={}", self.log_level))?;
            if self.json_logs {
                fmt()
                    .with_env_filter(filter)
                    .with_writer(reporter.make_writer())
                    .json()
                    .try_init()?;
            } else {
                fmt()
                    .with_env_filter(filter)
                    .with_writer(reporter.make_writer())
                    .try_init()?;
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
        assert_eq!(cli.jobs, default_jobs());
        assert_eq!(cli.progress, ProgressMode::Auto);
        assert!(!cli.flat && !cli.no_copy && !cli.watch);
    }
    #[test]
    fn supported_ext_does_not_require_input() {
        assert!(Cli::try_parse_from(["unlockm", "--supported-ext"]).is_ok());
    }

    #[test]
    fn typed_default_options_match_clap_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("track.ncm");
        std::fs::write(&input, b"x").unwrap();
        let input_string = input.to_string_lossy().into_owned();

        let cli = Cli::try_parse_from(["unlockm", &input_string]).unwrap();
        let from_cli = cli.into_config(ProgressReporter::disabled()).unwrap();
        let from_options = RunOptions::new(input).build().unwrap();

        assert_eq!(from_cli.input, from_options.input);
        assert_eq!(from_cli.output, from_options.output);
        assert_eq!(from_cli.layout, from_options.layout);
        assert_eq!(from_cli.copy_passthrough, from_options.copy_passthrough);
        assert_eq!(from_cli.overwrite, from_options.overwrite);
        assert_eq!(from_cli.watch, from_options.watch);
        assert_eq!(from_cli.update_metadata, from_options.update_metadata);
        assert_eq!(from_cli.jobs, from_options.jobs);
        assert_eq!(from_cli.buffer_size, from_options.buffer_size);
        assert_eq!(from_cli.metadata_jobs, from_options.metadata_jobs);
    }
}
