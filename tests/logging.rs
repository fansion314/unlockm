use std::{fs, process::Command};
use tempfile::tempdir;
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_unlockm"))
}
#[test]
fn default_warn_logs_file_failure() {
    let d = tempdir().unwrap();
    let p = d.path().join("bad.ncm");
    fs::write(&p, b"bad").unwrap();
    let out = bin().arg(&p).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("file processing failed"), "{text}");
}
#[test]
fn json_logs_emit_json_event() {
    let d = tempdir().unwrap();
    let p = d.path().join("bad.ncm");
    fs::write(&p, b"bad").unwrap();
    let out = bin()
        .args(["--json-logs", "--log-level", "warn"])
        .arg(&p)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with('{') && l.contains("file processing failed")),
        "{text}"
    );
}
#[test]
fn debug_level_is_accepted() {
    let d = tempdir().unwrap();
    let p = d.path().join("x.lrc");
    fs::write(&p, b"x").unwrap();
    assert!(bin()
        .args(["--log-level", "debug"])
        .arg(&p)
        .status()
        .unwrap()
        .success());
}
