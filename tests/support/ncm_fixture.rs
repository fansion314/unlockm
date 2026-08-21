//! Deterministic NCM inputs shared by metadata integration tests and release
//! benchmarks. The NCM header is the MIT/Apache-2.0 upstream
//! `ncm_test1.bin` parser vector; the payload is the repository's licensed
//! silent MP3 fixture. Keeping both here makes metadata benchmarks local and
//! repeatable after dependencies are fetched. Its fixed upstream header has a
//! retained static 256-byte cipher-stream vector, so this test/benchmark helper
//! has no dependency on the production crypto implementation.

use base64::{engine::general_purpose::STANDARD, Engine};
use std::{fs, path::Path};

pub const VALID_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLxmwAAAABJRU5ErkJggg==";

// MIT/Apache-2.0 upstream `ncm_test1.bin`, encoded as text so the fixture is
// self-contained and remains portable in source control.
const NCM_HEADER: &str = "Q1RFTkZEQU0BaZAAAAAsztXraer7FFUNRb9h3Rcdk3FHHuHd2vTV6E8cugAgwwLp/imS4YFFbxjHLRHyvFu83CIz+Wi0sCg4P2NsiGY1+eexcA7uVay47YtIFyU65l61gHiKzdzh7z0w7JwqxsdRrj0RtWSIntZ3ZvYrUp76+WP23icQRYKsLSCElUwPeq6LkW0QLmMc6sr5FJfYs+hWAwAAUlVQQwgGGksnDA1EF0MODAcKBRpKWS9VVyU2UDRXOhs7UDklNw4BOUhbTAUJBytVIgBIMTJMFwcNJBo5ESZaFSkkUAIHIFYtM1oMECkmIihMGg0LBigNLBsJLgsrKAITDy8sDiUbTCtSMzYJNy0LJScsDA0CUTVaGjUtLztUKg1MKxYbDCYhC1IlUjZbUVBWAQoZFFtXMgcGUCVVVFMwLDJSFQBQNzkoDhUPNkgRJDNWByIgKRQoBhYBVEgLFyAlNgIBFgcnFgw3ICokISJQVg40UjcUKRAyIDYtUiECCTsLARcAMzYNASErF0w5AjNRAiAHN0xaJTkJUSVQVEwZMjcIBjJVG1MLC0wIFjNUJxECGjAoVyVQOy4oNFE7NhFTElYJISssFC0kWgJTFiEpFgJTAioxLTFWOxdRUg9WUFYAF1cvCBpSJRItSAUCNw82MAUUWjdaUFUWCVM7GyQaGiFQBjcuVwk5FxMmMQRSUFUQU1JXAgQCLwFaAiovGUwMVQBQLSkGFhEgDQFTIjIbNAAKBgIVDyVaGiIRDiEyMSI5VloJFjUWMCkPMCcsVVISSBYPDDYMJScoNQoaBgkWORsIECcSLCxbSAFIM0gOG1QbAjYlIEgxCwYNFREXASQgJzEwIg8iCBcPEwwMGhISUUw7Uzs3ORYlBVQQMg1IDFBbIAsZIhkNCygsCwU7UQlUCgYPVwhRUCQpFlczNhUEUFsvUVYSCS8nMFErN1ckNVsMDg0NUyolBjMiKVEkCFE6Bg4WMTYrEDQwCAovGTJaBDlSNhomFA4LKkw5VhoBLxQzM1YWJSkzDgstGhEpCBUCK0wBDAcPFUw2NglME1MRERICDxFWK1shKQtRCQI7EQIkFhIECjcLIloLAgoCNjYqKRoUFBMbLBExKkwoESsbMFE1EVMNCQ85DyUXAlsaOSZbUTsuBC4hNgQ7EVYKLi0aJyEQB1MRNwcvUVcrJFcBUzVSDy8tSFJWKwAXGjkAEgkCBwJSLCBMOUxIUA0PFUwAKw83SBkXMjUxKAsNLTIQMlYzKDsrVRIEB1AkAAgBECYwLDQQIQ0lAi5TESsZNjEuWlIzNSAUUzAEBQg0EwJUIgVaASATFDkqJRs3SFdTKiI6ADRTJBJGV2xHAQwAAAAFAAAAaW1nIzFJTUFHRSMyLmTDX6yvG3QMVRTuwFBF";

// MIT/Apache-2.0 upstream `ncm_test1.bin` stream vector. It belongs to the
// fixed header above and lets external integration tests synthesize arbitrary
// payload lengths without importing `umc_ncm` or exposing crate-private code.
const AUDIO_KEY_STREAM: [u8; 256] = [
    0x67, 0x20, 0xf0, 0x5c, 0xac, 0xaf, 0x1b, 0x74, 0x0d, 0x26, 0x40, 0xbe, 0x85, 0x61, 0x45, 0x0d,
    0xa8, 0x69, 0xae, 0x78, 0xec, 0xb8, 0x14, 0x79, 0x53, 0xc4, 0x74, 0xf2, 0x9f, 0xe1, 0x18, 0xe2,
    0xf9, 0xc0, 0x7d, 0x1e, 0x5a, 0xba, 0x4a, 0xb3, 0x11, 0x36, 0xd9, 0xaa, 0xd2, 0x13, 0xee, 0x92,
    0x45, 0x4b, 0x57, 0xe6, 0xf1, 0x13, 0x90, 0xe9, 0xe5, 0x2f, 0xbe, 0x6e, 0x16, 0x01, 0xbf, 0x10,
    0x81, 0x5c, 0x68, 0x1c, 0xe3, 0xeb, 0xdf, 0x5a, 0xc7, 0xc3, 0x3f, 0x9c, 0xd7, 0x28, 0x3a, 0x81,
    0x2b, 0x1f, 0xf3, 0x6f, 0x27, 0x15, 0x7e, 0x53, 0xd3, 0xf0, 0xfa, 0x50, 0x9f, 0xc7, 0xf0, 0x7f,
    0xa6, 0xa7, 0x24, 0x80, 0x87, 0x39, 0x55, 0xa5, 0x0b, 0x2b, 0x8b, 0x00, 0x19, 0x82, 0xa8, 0x40,
    0xa5, 0x77, 0x9c, 0x93, 0x3b, 0xee, 0x27, 0x70, 0x0f, 0xa2, 0xeb, 0xa5, 0x6f, 0x58, 0xfb, 0xeb,
    0x57, 0x1b, 0xc3, 0x61, 0x10, 0x6d, 0x95, 0x2c, 0xaa, 0xc6, 0x58, 0x88, 0xd5, 0x9e, 0x33, 0x9a,
    0x5a, 0xd1, 0xb5, 0xd2, 0x17, 0x44, 0xd6, 0xdb, 0xab, 0xed, 0x7c, 0xfc, 0x5c, 0x37, 0xb9, 0x16,
    0xb8, 0xa5, 0x77, 0xfb, 0x58, 0x35, 0x36, 0xe7, 0x51, 0xd0, 0x03, 0xbb, 0x2a, 0x88, 0x24, 0x84,
    0x1d, 0x28, 0xb1, 0xe1, 0xa0, 0xa5, 0xa0, 0xdd, 0x15, 0x66, 0xbb, 0x4f, 0x7e, 0xbc, 0x99, 0x76,
    0x1a, 0xd2, 0xef, 0xbe, 0xd9, 0x79, 0xa0, 0x33, 0xd5, 0x96, 0x6c, 0xd9, 0xef, 0x42, 0x7e, 0x11,
    0x45, 0x1f, 0x99, 0x96, 0x1a, 0x90, 0x0c, 0x75, 0xbc, 0x01, 0xe2, 0xc8, 0xef, 0x16, 0x98, 0x9a,
    0x09, 0xeb, 0xd8, 0xa1, 0xea, 0x62, 0xe7, 0x92, 0x20, 0x8f, 0x6f, 0x72, 0xde, 0x14, 0xfe, 0xbf,
    0xcd, 0xba, 0x97, 0xde, 0xa3, 0x3b, 0x67, 0xd4, 0x0b, 0xf4, 0xd3, 0x97, 0x25, 0xba, 0xca, 0x08,
];

pub fn fixture_mp3() -> Vec<u8> {
    STANDARD
        .decode(
            fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lofty-silent.mp3.b64"),
            )
            .expect("read silent MP3 fixture")
            .trim(),
        )
        .expect("decode silent MP3 fixture")
}

pub fn ncm_with_audio(cover: &[u8], audio: &[u8]) -> Vec<u8> {
    // Combine the local upstream key/metadata bytes with a deterministic
    // playable payload. NCM's fixed stream transform is symmetric.
    let mut header = STANDARD.decode(NCM_HEADER).expect("decode NCM header");
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
    let mut encrypted_audio = audio.to_vec();
    for (byte, index) in encrypted_audio.iter_mut().zip(0..) {
        *byte ^= AUDIO_KEY_STREAM[index % AUDIO_KEY_STREAM.len()];
    }
    header.extend_from_slice(&encrypted_audio);
    header
}

pub fn mpeg_payload(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .windows(2)
        .position(|window| window[0] == 0xff && window[1] & 0xe0 == 0xe0)
        .expect("fixture has an MPEG frame");
    &bytes[start..]
}

/// Repeat a valid MPEG-frame sequence while retaining the fixture's single
/// ID3/header prefix. This is deliberately deterministic and uses no fake
/// container bytes, so Lofty still opens it through the normal MP3 path.
#[allow(dead_code)] // Included by metadata tests as well as the benchmark that uses it.
pub fn large_mp3(bytes: usize) -> Vec<u8> {
    let fixture = fixture_mp3();
    let frame_start = fixture.len() - mpeg_payload(&fixture).len();
    let target = bytes.max(fixture.len());
    let mut output = fixture[..frame_start].to_vec();
    let payload = &fixture[frame_start..];
    while output.len() < target {
        let remaining = target - output.len();
        output.extend_from_slice(&payload[..remaining.min(payload.len())]);
    }
    output
}
