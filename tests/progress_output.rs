use std::{fs, process::Command};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_unlockm"))
}

#[test]
fn redirected_auto_and_never_keep_stable_summary_without_control_sequences() {
    for mode in [None, Some("never")] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("song.lrc");
        fs::write(&input, b"words").unwrap();
        let mut command = bin();
        command.arg("-o").arg(temp.path().join("out"));
        if let Some(mode) = mode {
            command.args(["--progress", mode]);
        }
        let output = command.arg(&input).output().unwrap();
        assert!(output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains('\x1b') && !stderr.contains('\r'),
            "{stderr:?}"
        );
        assert!(
            stderr.contains("unlockm: converted=0, copied=1, skipped=0, conflicted=0, failed=0")
        );
    }
}

#[test]
fn always_mode_renders_two_line_activity_unicode_truncation_and_friendly_summary() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp
        .path()
        .join("目录-とても長い曲名-用于进度测试-さらに長い名前-第二部分.lrc");
    fs::write(&input, b"words").unwrap();
    let output = bin()
        .args(["--progress", "always", "-o"])
        .arg(temp.path().join("out"))
        .env("COLUMNS", "34")
        .arg(&input)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("\x1b[2K"), "{stderr:?}");
    assert!(stderr.contains("Copying:"), "{stderr:?}");
    assert!(stderr.contains('…'), "{stderr:?}");
    assert!(stderr.contains("1/1 100.0%"), "{stderr:?}");
    assert!(
        stderr.contains("Done in") && stderr.contains("1 copied"),
        "{stderr:?}"
    );
    assert!(!stderr.contains("unlockm: converted="), "{stderr:?}");
}

#[test]
fn completed_files_advance_before_the_next_serial_job_starts() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("in");
    fs::create_dir(&input).unwrap();
    for name in ["first.lrc", "second.lrc", "third.lrc"] {
        fs::write(input.join(name), vec![b'x'; 1024 * 1024]).unwrap();
    }

    let output = bin()
        .args(["--progress", "always", "--jobs", "1", "-o"])
        .arg(temp.path().join("out"))
        .env("COLUMNS", "120")
        .arg(&input)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_completion = stderr.find("1/3").expect("first live completion");
    let copy_starts = ["first.lrc", "second.lrc", "third.lrc"].map(|name| {
        stderr
            .find(&format!("Copying: {name}"))
            .unwrap_or_else(|| panic!("missing copy start for {name}: {stderr:?}"))
    });
    assert!(
        copy_starts
            .iter()
            .any(|position| *position > first_completion),
        "all jobs started before the first completion was rendered: {stderr:?}"
    );
}

#[test]
fn progress_aware_warning_and_partial_summary_remain_readable() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("bad.ncm");
    fs::write(&input, b"bad").unwrap();
    let output = bin()
        .args(["--progress", "always", "-o"])
        .arg(temp.path().join("out"))
        .arg(&input)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("file processing failed"), "{stderr:?}");
    assert!(
        stderr.contains("Done in") && stderr.contains("1 failed"),
        "{stderr:?}"
    );
    assert!(stderr.contains("\x1b[2K"), "{stderr:?}");
}

#[test]
fn json_mode_has_no_live_controls_and_ends_with_parseable_summary() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("song.lrc");
    fs::write(&input, b"words").unwrap();
    let output = bin()
        .args(["--json-logs", "-o"])
        .arg(temp.path().join("out"))
        .arg(&input)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains('\x1b') && !stderr.contains('\r'),
        "{stderr:?}"
    );
    let lines = stderr.lines().collect::<Vec<_>>();
    assert!(!lines.is_empty());
    let summary: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    assert_eq!(summary["event"], "summary");
    assert_eq!(summary["copied"], 1);
    assert_eq!(summary["exit_code"], 0);
    assert!(
        lines
            .iter()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn json_rejects_forced_progress_contract() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("song.lrc");
    fs::write(&input, b"words").unwrap();
    let output = bin()
        .args(["--json-logs", "--progress", "always"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("--json-logs conflicts with --progress always")
    );
}

#[test]
fn forced_progress_config_error_does_not_emit_cursor_controls_before_start() {
    let output = bin()
        .args(["--progress", "always", "does-not-exist"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains('\x1b') && !stderr.contains('\r'),
        "{stderr:?}"
    );
}
