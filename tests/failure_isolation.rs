use std::{fs, process::Command};
use tempfile::tempdir;
#[test]
fn bad_encrypted_file_does_not_stop_passthrough() {
    let d = tempdir().unwrap();
    let input = d.path().join("in");
    let output = d.path().join("out");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("bad.ncm"), b"not-ncm").unwrap();
    fs::write(input.join("ok.lrc"), b"ok").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_unlockm"))
        .arg("-o")
        .arg(&output)
        .arg(&input)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(3));
    assert_eq!(fs::read(output.join("ok.lrc")).unwrap(), b"ok");
}
