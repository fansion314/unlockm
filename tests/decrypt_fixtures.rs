//! Deterministic adapter-boundary fixtures.  Full crypto vectors live with the
//! adapter unit tests because they need the streaming sink; this suite guards
//! public registry/report behavior and corrupt-input policy.
use std::path::Path;

#[test]
fn supported_report_is_complete_deduplicated_and_sorted() {
    let values = unlockm::formats::supported_extensions();
    let mut sorted = values.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(values, sorted);
    for suffix in [
        ".ncm",
        ".qmc0",
        ".qmcflac",
        ".bkcape",
        ".mflacm",
        ".kgm.flac",
        ".vpr.flac",
        ".kgg",
        ".kgma",
        ".kwm",
        ".xm",
        ".x2m",
        ".x3m",
        ".tm6",
    ] {
        assert!(
            values.iter().any(|value| value == suffix),
            "missing {suffix}"
        );
    }
}

#[test]
fn registered_uppercase_and_multidot_names_remain_fail_closed_when_corrupt() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["broken.KGM.FLAC", "broken.NCM", "broken.XM", "broken.TM6"] {
        let path = temp.path().join(name);
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(
            unlockm::formats::prepare(&path).is_err(),
            "{name} was not rejected"
        );
    }
    for (name, kind) in [
        ("broken.QMCFLAC", unlockm::formats::Kind::QmcV2),
        ("broken.QMC0", unlockm::formats::Kind::QmcV1),
    ] {
        let path = temp.path().join(name);
        std::fs::write(&path, b"corrupt").unwrap();
        let output = temp.path().join(format!("{name}.out"));
        let mut sink = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(output)
            .unwrap();
        assert!(unlockm::formats::decrypt_to(
            kind,
            &path,
            &mut sink,
            7,
            None,
            &unlockm::keys::KeySources::default()
        )
        .is_err());
    }
    assert_eq!(
        unlockm::formats::candidate(Path::new("Album.Track.VPR.FLAC")),
        Some(unlockm::formats::Kind::Kgm)
    );
}

#[test]
fn standard_flac_disguised_with_registered_suffix_is_passthrough() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("track.QMCFLAC");
    let mut flac = b"fLaC".to_vec();
    flac.resize(64, 0);
    std::fs::write(&path, flac).unwrap();
    assert!(matches!(
        unlockm::formats::prepare(&path).unwrap(),
        unlockm::formats::PreparedClassification::Passthrough
    ));
}
