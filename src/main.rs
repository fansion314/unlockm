use clap::Parser;
use unlockm::{app, cli::Cli, error::AppError};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if cli.supported_ext {
        println!("{}", unlockm::formats::supported_extensions().join("\n"));
        return;
    }
    if let Err(error) = cli.init_logging() {
        eprintln!("unlockm: failed to initialize logging: {error}");
        std::process::exit(1);
    }
    let config = match cli.into_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("unlockm: {error}");
            std::process::exit(AppError::exit_code(&error));
        }
    };
    match app::run(config).await {
        Ok(summary) => {
            eprintln!(
                "unlockm: converted={}, copied={}, skipped={}, conflicted={}, failed={}",
                summary.converted,
                summary.copied,
                summary.skipped,
                summary.conflicted,
                summary.failed
            );
            std::process::exit(summary.exit_code())
        }
        Err(error) => {
            eprintln!("unlockm: {error}");
            std::process::exit(AppError::exit_code(&error));
        }
    }
}
