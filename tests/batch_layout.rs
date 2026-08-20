use std::{fs, process::Command};
use tempfile::tempdir;

fn unlockm() -> Command {
    Command::new(env!("CARGO_BIN_EXE_unlockm"))
}

#[test]
fn mirrors_nested_companions_and_empty_directories() {
    let tree = tempdir().unwrap();
    let input = tree.path().join("input");
    let output = tree.path().join("output");
    fs::create_dir_all(input.join("nested/empty")).unwrap();
    fs::write(input.join("nested/song.lrc"), "words").unwrap();
    assert!(unlockm()
        .args(["-o"])
        .arg(&output)
        .arg(&input)
        .status()
        .unwrap()
        .success());
    assert_eq!(
        fs::read_to_string(output.join("nested/song.lrc")).unwrap(),
        "words"
    );
    assert!(output.join("nested/empty").is_dir());
}

#[test]
fn no_copy_leaves_companions_out() {
    let tree = tempdir().unwrap();
    let input = tree.path().join("in");
    let output = tree.path().join("out");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("x.lrc"), "x").unwrap();
    assert!(unlockm()
        .arg("--no-copy")
        .arg("-o")
        .arg(&output)
        .arg(&input)
        .status()
        .unwrap()
        .success());
    assert!(!output.join("x.lrc").exists());
}

#[test]
fn flat_conflict_returns_partial_failure() {
    let tree = tempdir().unwrap();
    let input = tree.path().join("in");
    let output = tree.path().join("out");
    fs::create_dir_all(input.join("a")).unwrap();
    fs::create_dir_all(input.join("b")).unwrap();
    fs::write(input.join("a/x.lrc"), "a").unwrap();
    fs::write(input.join("b/x.lrc"), "b").unwrap();
    assert_eq!(
        unlockm()
            .arg("--flat")
            .arg("-o")
            .arg(&output)
            .arg(&input)
            .status()
            .unwrap()
            .code(),
        Some(3)
    );
}

#[test]
fn flat_preflight_rejects_every_copy_and_conversion_claimant() {
    let tree = tempdir().unwrap();
    let input = tree.path().join("in");
    let output = tree.path().join("out");
    fs::create_dir_all(input.join("encrypted")).unwrap();
    fs::create_dir_all(input.join("plain")).unwrap();

    // QMC v1 is symmetric.  Encrypt a tiny, positively probeable FLAC header
    // so the preflight must decrypt and sniff `song.flac`, rather than infer it
    // from the `.qmc0` suffix.
    let mut plain = b"fLaC".to_vec();
    plain.resize(64, 0);
    let mut qmc = plain.clone();
    umc_qmc::v1::decrypt(&mut qmc, 0);
    let mut round_trip = qmc.clone();
    umc_qmc::v1::decrypt(&mut round_trip, 0);
    assert_eq!(round_trip, plain);
    assert_eq!(
        unlockm::formats::extension(&round_trip).as_deref(),
        Some("flac")
    );
    assert!(!unlockm::formats::is_standard_audio(&qmc));
    fs::write(input.join("encrypted/song.qmc0"), qmc).unwrap();
    fs::write(input.join("plain/song.flac"), b"fLaC companion audio").unwrap();

    let config = unlockm::config::RunConfig::new(
        input.clone(),
        Some(output.clone()),
        true,
        false,
        false,
        false,
        false,
        1,
        64,
        1,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let planned =
        unlockm::app::job::preflight(&config, &config.input.join("encrypted/song.qmc0")).unwrap();
    assert_eq!(
        planned.unwrap().destination,
        config.output.join("song.flac")
    );

    for overwrite in [false, true] {
        let mut command = unlockm();
        command.arg("--flat").arg("-o").arg(&output);
        if overwrite {
            command.arg("--overwrite");
        }
        let result = command.arg(&input).output().unwrap();
        assert_eq!(
            result.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            !output.join("song.flac").exists(),
            "same-run flat conflicts must not publish a winner: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            fs::read_dir(&output).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".unlockm-stage-")),
            "conflicted staging files are owned and cleaned"
        );
    }
}

#[test]
fn in_place_passthrough_is_a_noop() {
    let tree = tempdir().unwrap();
    let input = tree.path().join("in");
    fs::create_dir_all(&input).unwrap();
    let source = input.join("x.lrc");
    fs::write(&source, "x").unwrap();
    assert!(unlockm().arg(&input).status().unwrap().success());
    assert_eq!(fs::read_to_string(source).unwrap(), "x");
}

#[test]
fn copied_file_keeps_mtime() {
    let tree = tempdir().unwrap();
    let input = tree.path().join("in");
    let output = tree.path().join("out");
    fs::create_dir(&input).unwrap();
    let source = input.join("x.lrc");
    fs::write(&source, "x").unwrap();
    let time = filetime::FileTime::from_unix_time(1_700_000_000, 0);
    filetime::set_file_mtime(&source, time).unwrap();
    assert!(unlockm()
        .arg("-o")
        .arg(&output)
        .arg(&input)
        .status()
        .unwrap()
        .success());
    assert_eq!(
        filetime::FileTime::from_last_modification_time(
            &fs::metadata(output.join("x.lrc")).unwrap()
        ),
        time
    );
}
