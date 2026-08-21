use clap::Parser;
use std::time::Instant;
use unlockm::{app, cli::Cli, error::AppError};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if cli.supported_ext {
        println!("{}", unlockm::formats::supported_extensions().join("\n"));
        return;
    }
    let reporter = cli.progress_reporter();
    let json_logs = cli.json_logs;
    let interactive = reporter.is_interactive();
    if let Err(error) = cli.init_logging(reporter.clone()) {
        reporter.finish();
        eprintln!("unlockm: failed to initialize logging: {error}");
        std::process::exit(1);
    }
    let config = match cli.into_config(reporter.clone()) {
        Ok(config) => config,
        Err(error) => {
            reporter.finish();
            eprintln!("unlockm: {error}");
            std::process::exit(AppError::exit_code(&error));
        }
    };
    let started = Instant::now();
    match app::run(config).await {
        Ok(summary) => {
            reporter.finish();
            if json_logs {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "event": "summary",
                        "elapsed_ms": started.elapsed().as_millis(),
                        "converted": summary.converted,
                        "copied": summary.copied,
                        "skipped": summary.skipped,
                        "conflicted": summary.conflicted,
                        "failed": summary.failed,
                        "exit_code": summary.exit_code(),
                    })
                );
            } else if interactive {
                eprintln!(
                    "Done in {:.2}s — {} converted, {} copied, {} skipped, {} conflicted, {} failed",
                    started.elapsed().as_secs_f32(),
                    summary.converted,
                    summary.copied,
                    summary.skipped,
                    summary.conflicted,
                    summary.failed
                );
            } else {
                eprintln!(
                    "unlockm: converted={}, copied={}, skipped={}, conflicted={}, failed={}",
                    summary.converted,
                    summary.copied,
                    summary.skipped,
                    summary.conflicted,
                    summary.failed
                );
            }
            std::process::exit(summary.exit_code())
        }
        Err(error) => {
            reporter.finish();
            eprintln!("unlockm: {error}");
            std::process::exit(AppError::exit_code(&error));
        }
    }
}
