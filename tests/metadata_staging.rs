#[path = "support/ncm_fixture.rs"]
mod ncm_fixture;

use base64::{engine::general_purpose::STANDARD, Engine};
use ncm_fixture::{fixture_mp3, mpeg_payload, ncm_with_audio, VALID_PNG};
use std::{fs, path::Path, process::Command};

fn run(input: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_unlockm"))
        .args(["--update-metadata", "--log-level", "warn", "-o"])
        .arg(output)
        .arg(input)
        .output()
        .unwrap()
}

fn combined(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn synthetic_ncm_stages_extensionless_metadata_then_publishes_native_audio() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("synthetic.ncm");
    let output = temp.path().join("out");
    fs::write(
        &input,
        ncm_with_audio(&STANDARD.decode(VALID_PNG).unwrap(), &fixture_mp3()),
    )
    .unwrap();
    let result = run(&input, &output);
    let log = combined(&result);
    assert!(result.status.success(), "{log}");
    assert!(
        log.contains("converted=1") && log.contains("failed=0"),
        "{log}"
    );
    assert!(!log.contains("could not read tags"), "{log}");
    assert!(!log.contains("metadata update skipped"), "{log}");
    assert!(!log.contains("No format could be determined"), "{log}");
    let published = output.join("synthetic.mp3");
    let metadata = unlockm::metadata::tags::read(&published).unwrap();
    assert_eq!(
        metadata.cover.unwrap().bytes.unwrap(),
        STANDARD.decode(VALID_PNG).unwrap()
    );
    let output_bytes = fs::read(&published).unwrap();
    let source = fixture_mp3();
    assert_eq!(mpeg_payload(&output_bytes), mpeg_payload(&source));
    assert_eq!(
        fs::read_dir(&output)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".unlockm-"))
            .count(),
        0
    );
}

#[test]
fn invalid_ncm_art_is_best_effort_with_one_source_warning() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("invalid-cover.ncm");
    let output = temp.path().join("out");
    fs::write(&input, ncm_with_audio(b"not-an-image", &fixture_mp3())).unwrap();
    let result = run(&input, &output);
    let log = combined(&result);
    assert!(result.status.success(), "{log}");
    assert!(
        log.contains("converted=1") && log.contains("failed=0"),
        "{log}"
    );
    assert_eq!(log.matches("metadata update skipped").count(), 1, "{log}");
    assert_eq!(log.matches("could not read tags").count(), 0, "{log}");
    assert!(output.join("invalid-cover.mp3").is_file());
}
