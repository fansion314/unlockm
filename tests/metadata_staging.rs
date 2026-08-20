use base64::{engine::general_purpose::STANDARD, Engine};
use std::{fs, path::Path, process::Command};

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLxmwAAAABJRU5ErkJggg==";
// MIT/Apache-2.0 upstream `ncm_test1.bin`, encoded here so this staging
// regression is self-contained and does not need a sibling checkout.
const NCM_HEADER: &str = "Q1RFTkZEQU0BaZAAAAAsztXraer7FFUNRb9h3Rcdk3FHHuHd2vTV6E8cugAgwwLp/imS4YFFbxjHLRHyvFu83CIz+Wi0sCg4P2NsiGY1+eexcA7uVay47YtIFyU65l61gHiKzdzh7z0w7JwqxsdRrj0RtWSIntZ3ZvYrUp76+WP23icQRYKsLSCElUwPeq6LkW0QLmMc6sr5FJfYs+hWAwAAUlVQQwgGGksnDA1EF0MODAcKBRpKWS9VVyU2UDRXOhs7UDklNw4BOUhbTAUJBytVIgBIMTJMFwcNJBo5ESZaFSkkUAIHIFYtM1oMECkmIihMGg0LBigNLBsJLgsrKAITDy8sDiUbTCtSMzYJNy0LJScsDA0CUTVaGjUtLztUKg1MKxYbDCYhC1IlUjZbUVBWAQoZFFtXMgcGUCVVVFMwLDJSFQBQNzkoDhUPNkgRJDNWByIgKRQoBhYBVEgLFyAlNgIBFgcnFgw3ICokISJQVg40UjcUKRAyIDYtUiECCTsLARcAMzYNASErF0w5AjNRAiAHN0xaJTkJUSVQVEwZMjcIBjJVG1MLC0wIFjNUJxECGjAoVyVQOy4oNFE7NhFTElYJISssFC0kWgJTFiEpFgJTAioxLTFWOxdRUg9WUFYAF1cvCBpSJRItSAUCNw82MAUUWjdaUFUWCVM7GyQaGiFQBjcuVwk5FxMmMQRSUFUQU1JXAgQCLwFaAiovGUwMVQBQLSkGFhEgDQFTIjIbNAAKBgIVDyVaGiIRDiEyMSI5VloJFjUWMCkPMCcsVVISSBYPDDYMJScoNQoaBgkWORsIECcSLCxbSAFIM0gOG1QbAjYlIEgxCwYNFREXASQgJzEwIg8iCBcPEwwMGhISUUw7Uzs3ORYlBVQQMg1IDFBbIAsZIhkNCygsCwU7UQlUCgYPVwhRUCQpFlczNhUEUFsvUVYSCS8nMFErN1ckNVsMDg0NUyolBjMiKVEkCFE6Bg4WMTYrEDQwCAovGTJaBDlSNhomFA4LKkw5VhoBLxQzM1YWJSkzDgstGhEpCBUCK0wBDAcPFUw2NglME1MRERICDxFWK1shKQtRCQI7EQIkFhIECjcLIloLAgoCNjYqKRoUFBMbLBExKkwoESsbMFE1EVMNCQ85DyUXAlsaOSZbUTsuBC4hNgQ7EVYKLi0aJyEQB1MRNwcvUVcrJFcBUzVSDy8tSFJWKwAXGjkAEgkCBwJSLCBMOUxIUA0PFUwAKw83SBkXMjUxKAsNLTIQMlYzKDsrVRIEB1AkAAgBECYwLDQQIQ0lAi5TESsZNjEuWlIzNSAUUzAEBQg0EwJUIgVaASATFDkqJRs3SFdTKiI6ADRTJBJGV2xHAQwAAAAFAAAAaW1nIzFJTUFHRSMyLmTDX6yvG3QMVRTuwFBF";

fn fixture_mp3() -> Vec<u8> {
    STANDARD
        .decode(
            fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lofty-silent.mp3.b64"),
            )
            .unwrap()
            .trim(),
        )
        .unwrap()
}

fn ncm_with_audio(cover: &[u8]) -> Vec<u8> {
    // MIT/Apache-2.0 upstream parser vector; the resulting file combines its
    // key/metadata container with the licensed deterministic silent MP3.
    let mut header = STANDARD.decode(NCM_HEADER).unwrap();
    let original = umc_ncm::header::NCMFile::new(&header).unwrap();
    let mut offset = 10;
    let content_len = u32::from_le_bytes(header[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4 + content_len;
    let metadata_len = u32::from_le_bytes(header[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4 + metadata_len + 4 + 1;
    let frame_len = offset;
    let image_len = offset + 4;
    let image_start = offset + 8;
    header[frame_len..frame_len + 4].copy_from_slice(&(cover.len() as u32).to_le_bytes());
    header[image_len..image_len + 4].copy_from_slice(&(cover.len() as u32).to_le_bytes());
    header.truncate(image_start);
    header.extend_from_slice(cover);
    let mut audio = fixture_mp3();
    original.decrypt(&mut audio, 0); // NCM's stream transform is symmetric.
    header.extend_from_slice(&audio);
    header
}

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
    fs::write(&input, ncm_with_audio(&STANDARD.decode(PNG).unwrap())).unwrap();
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
        STANDARD.decode(PNG).unwrap()
    );
    let output_bytes = fs::read(&published).unwrap();
    let start = output_bytes
        .windows(2)
        .position(|v| v[0] == 0xff && v[1] & 0xe0 == 0xe0)
        .unwrap();
    let source = fixture_mp3();
    let source_start = source
        .windows(2)
        .position(|v| v[0] == 0xff && v[1] & 0xe0 == 0xe0)
        .unwrap();
    assert_eq!(&output_bytes[start..], &source[source_start..]);
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
    fs::write(&input, ncm_with_audio(b"not-an-image")).unwrap();
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
