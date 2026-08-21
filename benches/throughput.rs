//! Stable, dependency-free release benchmark for the hot QMCv1 path.
//!
//! The payload is deterministic synthetic FLAC-looking bytes: it is never a
//! recording and it exercises the same offset-aware pinned cipher and staging
//! pipeline as a real QMCv1 input. `cargo bench` prints hashes and throughput;
//! change BENCH_BYTES/BENCH_FILES/BENCH_SAMPLES for slower machines.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use unlockm::{app, config::RunConfig};

#[path = "../tests/support/qmc_fixture.rs"]
mod qmc_fixture;

const BUFFER: usize = 1024 * 1024;

fn env_bytes(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn generated_byte(offset: usize) -> u8 {
    ((offset as u64).wrapping_mul(0x9e37_79b9).rotate_left(11) >> 24) as u8
}
fn write_qmc(path: &Path, bytes: usize) -> [u8; 32] {
    let mut output = File::create(path).unwrap();
    let mut hash = Sha256::new();
    let mut offset = 0;
    let mut chunk = vec![0; BUFFER];
    while offset < bytes {
        let n = (bytes - offset).min(chunk.len());
        for (index, byte) in chunk[..n].iter_mut().enumerate() {
            *byte = match offset + index {
                0 => b'f',
                1 => b'L',
                2 => b'a',
                3 => b'C',
                4..=63 => 0,
                position => generated_byte(position),
            };
        }
        hash.update(&chunk[..n]);
        qmc_fixture::transform(&mut chunk[..n], offset);
        output.write_all(&chunk[..n]).unwrap();
        offset += n;
    }
    hash.finalize().into()
}
fn expected_hash(bytes: usize) -> [u8; 32] {
    let mut hash = Sha256::new();
    let mut offset = 0;
    let mut chunk = vec![0; BUFFER];
    while offset < bytes {
        let n = (bytes - offset).min(chunk.len());
        for (index, byte) in chunk[..n].iter_mut().enumerate() {
            *byte = match offset + index {
                0 => b'f',
                1 => b'L',
                2 => b'a',
                3 => b'C',
                4..=63 => 0,
                position => generated_byte(position),
            };
        }
        hash.update(&chunk[..n]);
        offset += n;
    }
    hash.finalize().into()
}
fn hash(path: &Path) -> [u8; 32] {
    let mut input = File::open(path).unwrap();
    let mut hash = Sha256::new();
    let mut chunk = vec![0; BUFFER];
    loop {
        let n = input.read(&mut chunk).unwrap();
        if n == 0 {
            break;
        }
        hash.update(&chunk[..n]);
    }
    hash.finalize().into()
}
fn direct(input: &Path, output: &Path) -> Duration {
    let start = Instant::now();
    let mut input = File::open(input).unwrap();
    let mut output = File::create(output).unwrap();
    let mut chunk = vec![0; BUFFER];
    let mut offset = 0;
    loop {
        let n = input.read(&mut chunk).unwrap();
        if n == 0 {
            break;
        }
        qmc_fixture::transform(&mut chunk[..n], offset);
        output.write_all(&chunk[..n]).unwrap();
        offset += n;
    }
    output.flush().unwrap();
    start.elapsed()
}
fn pipeline(input: PathBuf, output: PathBuf, jobs: usize) -> Duration {
    let config = Arc::new(
        RunConfig::new(
            input.clone(),
            Some(output),
            false,
            false,
            true,
            false,
            false,
            jobs,
            BUFFER,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap(),
    );
    let input = config.input.clone();
    let start = Instant::now();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let summary = runtime
        .block_on(app::runner::run_paths(config, vec![input]))
        .unwrap();
    assert_eq!(summary.converted, 1, "pipeline summary: {summary:?}");
    start.elapsed()
}
fn pipeline_many(input: PathBuf, output: PathBuf, jobs: usize, files: usize) -> Duration {
    let config = Arc::new(
        RunConfig::new(
            input.clone(),
            Some(output),
            false,
            false,
            true,
            false,
            false,
            jobs,
            BUFFER,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap(),
    );
    let paths = (0..files)
        .map(|n| config.input.join(format!("song-{n}.qmc0")))
        .collect();
    let start = Instant::now();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let summary = runtime
        .block_on(app::runner::run_paths(config, paths))
        .unwrap();
    assert_eq!(summary.converted, files);
    start.elapsed()
}
fn report(name: &str, bytes: usize, elapsed: Duration, expected: &[u8; 32], output: &Path) {
    assert_eq!(&hash(output), expected, "{name} output hash");
    let mib_s = bytes as f64 / 1024.0 / 1024.0 / elapsed.as_secs_f64();
    println!(
        "{name}: bytes={bytes} seconds={:.3} MiB/s={mib_s:.2} sha256={:02x?}",
        elapsed.as_secs_f64(),
        &expected[..4]
    );
}
fn main() {
    let bytes = env_bytes("UNLOCKM_BENCH_BYTES", 64 * 1024 * 1024).max(4096);
    let files = env_bytes("UNLOCKM_BENCH_FILES", 8).max(2);
    let samples = env_bytes("UNLOCKM_BENCH_SAMPLES", 1).max(1);
    let jobs = std::thread::available_parallelism().map_or(1, usize::from);
    let directory = tempfile::tempdir().unwrap();
    let single = directory.path().join("single.qmc0");
    let expected = write_qmc(&single, bytes);
    let many_input = directory.path().join("many-in");
    fs::create_dir(&many_input).unwrap();
    let many_expected = expected_hash(bytes / files);
    for n in 0..files {
        assert_eq!(
            write_qmc(&many_input.join(format!("song-{n}.qmc0")), bytes / files),
            many_expected
        );
    }
    println!(
        "unlockm throughput: bytes={bytes} files={files} jobs={jobs} samples={samples} buffer={BUFFER}"
    );
    for sample in 0..samples {
        let direct_out = directory.path().join(format!("direct-{sample}.flac"));
        let elapsed = direct(&single, &direct_out);
        report("direct-qmcv1", bytes, elapsed, &expected, &direct_out);
        let one_out = directory.path().join(format!("one-{sample}"));
        let elapsed = pipeline(single.clone(), one_out.clone(), 1);
        report(
            "pipeline-one-jobs1",
            bytes,
            elapsed,
            &expected,
            &one_out.join("single.flac"),
        );
        let many_one = directory.path().join(format!("many-one-{sample}"));
        let one_elapsed = pipeline_many(many_input.clone(), many_one.clone(), 1, files);
        assert_many_hashes(&many_one, files, &many_expected);
        println!(
            "pipeline-many-jobs1: bytes={bytes} seconds={:.3} MiB/s={:.2}",
            one_elapsed.as_secs_f64(),
            bytes as f64 / 1024.0 / 1024.0 / one_elapsed.as_secs_f64()
        );
        let many_default = directory.path().join(format!("many-default-{sample}"));
        let default_elapsed = pipeline_many(many_input.clone(), many_default.clone(), jobs, files);
        assert_many_hashes(&many_default, files, &many_expected);
        println!(
            "pipeline-many-default: bytes={bytes} seconds={:.3} MiB/s={:.2} scaling={:.2}x",
            default_elapsed.as_secs_f64(),
            bytes as f64 / 1024.0 / 1024.0 / default_elapsed.as_secs_f64(),
            one_elapsed.as_secs_f64() / default_elapsed.as_secs_f64()
        );
    }
}
fn assert_many_hashes(output: &Path, files: usize, expected: &[u8; 32]) {
    for n in 0..files {
        assert_eq!(hash(&output.join(format!("song-{n}.flac"))), *expected);
    }
}
