# ARCHITECTURE.md

## 1. Project Overview

`unlockm` is a cross-platform Rust CLI that turns encrypted files downloaded by music applications into standard audio files while preserving directory layout, companion files, metadata, and album art.

The first release supports one-shot file/directory conversion and a long-running recursive watch mode. It targets Windows, macOS, and Linux, favors high aggregate throughput, keeps memory bounded, and never requires audio transcoding.

The repository is currently in its design phase. The paths below describe the target implementation layout. The behavioral references are:

- Go CLI `../cli` at `d0d3647d615c8624abf27e30ade07d23da757041`, especially its resilient recursive processing and passthrough behavior.
- Rust crypto workspace `../lib_um_crypto_rust` at `5dad838c33c3e4aab198d995160d3fbb9968470f` (`v0.1.12`) for tested cipher and audio-sniffing primitives.

Non-goals for the first release are audio transcoding, a GUI, library-management features, automatic source deletion, and writing metadata obtained from an ambiguous network search.

## 2. High-Level Architecture

The CLI is a thin configuration boundary. A shared application pipeline serves initial scans, direct file conversion, and watch events.

```text
CLI/config -> scan or watch -> path planner -> bounded job runner
                                              |             |
                                      format adapter    passthrough copy
                                              |
                                      streaming decrypt
                                              |
                                  conservative metadata merge
                                              |
                                    atomic output publish
```

Tokio owns cancellation, filesystem events, network metadata requests, and task coordination. File scanning, decryption, copying, audio probing, and tag writes are blocking operations and run behind a concurrency semaphore on `spawn_blocking`. Each file is processed sequentially, while independent files run concurrently.

This hybrid model avoids blocking the async runtime without forcing synchronous cipher APIs into artificial async wrappers. It also limits simultaneous open files, buffers, metadata requests, and memory use.

## 3. Code Map

The planned single-package layout keeps a reusable library surface in `src/lib.rs` and a small executable in `src/main.rs`.

- `src/main.rs`: parse process-level errors into exit codes and install signal handling.
- `src/cli.rs`: Clap arguments and conversion into validated `RunConfig`. Change user-facing flags here.
- `src/config.rs`: normalized input/output roots, `OutputLayout`, copy/overwrite policy, concurrency limits, key-source settings, and timeouts.
- `src/app/`: shared orchestration.
  - `scan.rs`: recursive initial enumeration, empty-directory mirroring, and output-subtree pruning.
  - `watch.rs`: recursive notifications, debouncing, file-stability checks, per-path deduplication, and requeue-on-change.
  - `runner.rs`: bounded scheduling, cancellation, summaries, and batch/watch error policy.
  - `job.rs`: the per-file state machine from classification through atomic publication.
- `src/pathing.rs`: all relative-path and destination calculations, including mirror/flat layouts and collision detection.
- `src/formats/`: encrypted-format registry and adapters. Change cipher dispatch here, not in the CLI.
  - `ncm.rs`, `qmc.rs`, `kgm.rs`, `kwm.rs`, `xiami.rs`, `ximalaya.rs`, and `tm.rs` implement Go-reference parity.
  - Additional adapters for the Rust library's JOOX, Migu 3D, and Qingting FM formats can use the same trait without changing orchestration.
  - `audio.rs` recognizes already-standard audio so encrypted-looking suffixes are not blindly decrypted or copied.
- `src/keys/`: QMC key maps/MMKV discovery and KGM v5 database lookup. Key acquisition is separate from cipher adapters.
- `src/metadata/`: typed tags, provenance, merge rules, cover validation, tag read/write, and provider clients.
  - `provider/qq.rs` owns QQ Music RPC/search/cover requests.
  - NCM container metadata is parsed locally; its cover URL is the only NCM network request needed.
- `src/io/atomic.rs`: same-directory temporary outputs, flush/close, overwrite policy, and final rename.
- `src/error.rs`: typed library errors and user-facing context.
- `tests/fixtures/`: small legal/synthetic encrypted fixtures and expected outputs.
- `tests/`: CLI, directory-layout, watch, metadata, failure-isolation, and end-to-end tests.
- `benches/`: streaming and multi-file throughput benchmarks used to tune buffers and default concurrency.

## 4. Main Execution Flow

1. `cli` parses `unlockm [OPTIONS] <INPUT>`. `RunConfig` canonicalizes paths, defaults output to the input directory (or a file's parent), and rejects invalid combinations.
2. `scan` emits file entries and, in mirror mode, empty directories. A proper output subtree inside the input is pruned to prevent recursive `output/output` processing.
3. `pathing` computes the destination before work starts. Mirror mode preserves the input-relative path; `--flat` uses only the basename. Decrypted audio receives its sniffed standard extension.
4. `formats` first selects candidates by longest, case-insensitive suffix, then validates headers. Standard audio is passthrough; an invalid encrypted-looking file is an error unless standard-audio probing positively identifies it.
5. `runner` admits at most `--jobs` blocking jobs. A decrypt adapter streams payload chunks through the pinned `umc_*` cipher into a temporary file in the destination directory. Copies use the platform's optimized filesystem copy where possible.
6. The decrypted header is probed with `um_audio` to choose the real output extension. A mismatch with an early path prediction is resolved before destination reservation and publication.
7. Without `--update-metadata`, decrypted audio bytes and any existing embedded tags are retained. With it, `metadata` reads existing tags, merges only verified source-specific data, validates cover bytes, and writes tags without re-encoding audio.
8. `atomic` publishes the completed temporary file. Existing outputs are skipped unless `--overwrite` is set. A failure leaves the source and any pre-existing output untouched.
9. Batch mode continues after per-file failures and returns a non-zero status after printing a summary. Watch mode logs per-file failures and stays alive until cancellation or a fatal watcher/configuration error.

Watch mode performs the same initial scan, then converts normalized create, modify, and rename-to events through steps 3-8. Events are debounced and a file must have stable size and modification time across consecutive checks before it enters the runner. Rename-from/remove cancels queued work (and prevents a running job from publishing); a low-frequency reconciliation resubmits visible sources to cover notification loss without creating a second processing pipeline.

## 5. Architectural Boundaries

- `cli` depends on application/configuration types; it never calls a cipher, network provider, tag writer, or watcher directly.
- `app` coordinates traits and policies. It does not contain format-specific keys, suffix lists, or metadata-provider response models.
- `formats` may depend on `umc_*`, `um_audio`, and `keys`; it does not decide destination paths or overwrite behavior.
- `metadata` cannot decrypt files or publish outputs. Providers return typed candidates with identity evidence and never write tags themselves.
- `pathing` is pure apart from filesystem normalization. It does not probe file content.
- Only `io::atomic` may replace a destination. Adapters must write to the provided temporary sink and must not create final paths.
- Production modules must not import test fixtures or benchmark helpers.

## 6. Architectural Invariants

- All entry paths are normalized before scanning; all destinations are derived from an input-relative path and remain under the configured output root.
- Mirror layout and flat layout use the same planner in batch and watch modes.
- Mirror mode plus default copy policy preserves nested companion files and empty directories. `--no-copy` disables only passthrough copies, not encrypted-audio conversion.
- Flat-mode destination collisions are reported as errors. `--overwrite` permits replacing an existing destination but never chooses a winner between two sources in the same run.
- Unknown files may be copied; corrupt or unsupported files with a registered encrypted suffix are not silently copied as successful conversions.
- Decryption and copying are streaming and bounded by `jobs * buffer_size`, aside from small headers, format footers, tags, covers, and key indexes.
- The destination becomes visible only after decryption, optional metadata updates, and validation succeed.
- Sources are never removed in the first release.
- Network failure cannot make decryption fail. It can only leave metadata unchanged and produce a warning.
- Ambiguous metadata is never committed. Existing tags and covers win over guesses.
- Watch events for a path are coalesced; if a file changes during processing, exactly one follow-up run is scheduled. Watch jobs share the batch runner's job semaphore, key indexes, metadata cache, and output reservations.

## 7. Cross-Cutting Concerns

### Concurrency and performance

`--jobs` defaults to `std::thread::available_parallelism()`. A Tokio semaphore bounds blocking file jobs, while a smaller independent semaphore bounds metadata HTTP requests. The decrypt buffer defaults to 1 MiB and is configurable; benchmarks may justify changing the default without changing the architecture.

Cipher transforms operate in place with an absolute payload offset. No job loads an audio payload wholly into memory. Network responses and cover images have strict size and time limits. Key databases are parsed once and cached read-only for all workers.

### Error handling and diagnostics

Library modules return typed errors with source/destination and processing-stage context. `tracing` supplies warn-level human output by default, optional verbosity, and machine-readable JSON logs. Secrets and raw keys are always redacted.

Batch summaries count converted, copied, skipped, conflicted, and failed files. Expected per-file errors do not abort sibling work.

### Output safety

Temporary files are created beside their final destination so the final rename stays on one filesystem. They use a recognizable hidden prefix so scans/watchers can ignore them. Cancellation or failure removes only the owned temporary file.

Passthrough where source and destination are the same path is an explicit no-op. If output is a proper descendant of input, that subtree is excluded from both initial traversal and watcher events.

### Metadata correctness

Metadata uses field-level provenance rather than a single untyped record:

1. For title, album, and artists, NCM container JSON or an ID-addressed QQ Music response is authoritative.
2. Existing embedded audio tags are retained when no stronger identity-bound value exists.
3. Filenames are query hints only. A search result is eligible only when normalized title and artist agree, duration is within tolerance, and the best result has a safe margin over the runner-up.
4. For covers, existing embedded audio art is preferred, then container-embedded art, then exact-ID network art. Search-derived art follows the same strict identity gate.

Image magic/MIME, byte limit, and decodability are checked before embedding. Native Rust tag writing preserves audio frames and avoids an FFmpeg runtime dependency. If a container/tag combination cannot be safely updated, conversion succeeds with existing tags and a warning.

### Testing and release

Unit tests cover path planning, suffix precedence, metadata merging, output reservations, key lookup, and event coalescing. Integration tests use temporary trees and deterministic local HTTP fixtures. Watch tests wait on observable outputs rather than fixed sleeps. Release checks include format fixtures, `cargo fmt`, Clippy with warnings denied, tests, and optimized builds on Windows, macOS, and Linux.

## 8. Extension Points

- Add an encrypted format by implementing `FormatAdapter`, registering suffixes in `formats::registry`, and providing positive/negative/header-truncation fixtures. Orchestration and CLI code should not change.
- Add a key source by implementing `KeySource`; adapters ask by stable identifiers and never know whether a key came from a footer, file, MMKV store, or SQLite database.
- Add a metadata provider by implementing `MetadataProvider` and returning evidence-bearing candidates. Merge thresholds remain centralized.
- Add an output layout by extending `OutputLayout` and pure planner tests before changing scanning or watch behavior.
- Rust-library-only JOOX, Migu 3D, and Qingting FM support should be added after Go-reference parity through adapters, not subcommands.

## 9. Important Types and Concepts

- `RunConfig`: fully validated immutable configuration shared by every job.
- `OutputLayout`: `Mirror` by default or `Flat` with explicit collision errors.
- `CopyPolicy`: `Copy` by default or `Skip` from `--no-copy`.
- `JobSpec`: source identity, relative path, planned destination, and event generation.
- `JobOutcome`: converted/copied/skipped/conflicted/failed plus timings and byte counts.
- `FormatAdapter`: validates a candidate and prepares a streaming decrypt operation plus metadata hints.
- `PreparedDecrypt`: payload bounds, output-name stem, cipher operation, and source metadata evidence.
- `OutputReservation`: prevents two concurrent jobs from publishing the same path.
- `MetadataValue<T>`: a value plus provenance and identity confidence.
- `PathState`: watch-mode queued/running/dirty state for event coalescing.

## 10. External Dependencies

- `umc_*` and `um_audio` from `lib_um_crypto_rust` are pinned to revision `5dad838...`; they own cipher primitives, footer/header parsing, and audio type detection. Local development may use a Cargo patch, but committed builds must not depend on sibling checkout paths.
- Tokio owns async orchestration, cancellation, and HTTP/watch integration; it does not run blocking decrypt loops directly.
- Clap owns the public CLI grammar.
- `notify` (with a debouncer layer) owns platform filesystem notifications. Application code supplies stability and deduplication semantics.
- Reqwest owns bounded HTTPS access for metadata and covers.
- Serde owns NCM/QMC/provider payload parsing.
- Lofty owns native tag and picture reads/writes. It is an adapter boundary because container support may evolve.
- Tracing owns structured diagnostics.
- Tempfile or an equivalent small wrapper owns unique same-directory temporary files.
- A Rust SQLite binding is confined to KGM v5 key extraction after `umc_kgm::decrypt_db` decrypts the database bytes.

The Go source is MIT licensed; the Rust crypto reference is MIT/Apache-2.0 dual licensed. Any ported logic retains required notices and attribution.

## 11. Repository Layout

```text
unlockm/
├── AGENTS.md
├── ARCHITECTURE.md
├── Cargo.toml
├── README.md
├── docs/
│   ├── PLANS.md
│   └── plans/
├── src/
│   ├── app/
│   ├── formats/
│   ├── io/
│   ├── keys/
│   ├── metadata/
│   ├── cli.rs
│   ├── config.rs
│   ├── error.rs
│   ├── lib.rs
│   ├── main.rs
│   └── pathing.rs
├── tests/
│   └── fixtures/
└── benches/
```

## 12. Design Decisions

- **One modular package, not a multi-crate workspace.** This keeps initialization and releases simple while `src/lib.rs` preserves testability. Split crates only if a real second consumer appears.
- **Pin and wrap crypto crates instead of copying them.** This reuses tested algorithms and keeps application policy local. The tradeoff is an upstream git dependency and adapter work when APIs change.
- **Hybrid Tokio plus blocking workers.** Directory/watch/network concurrency benefits from async coordination; cipher and tag APIs remain efficient synchronous code. The tradeoff is two execution domains and explicit admission control.
- **In-place output is the compatibility default.** Omitting `--output` writes standard audio beside the source, as the Go CLI does. A separate output root activates useful passthrough copying and mirrored trees.
- **Mirror and copy are defaults.** `--flat` and `--no-copy` are explicit opt-outs. Flat collisions fail instead of silently overwriting or inventing names.
- **Atomic publish over direct writes.** Extra temporary disk space is accepted to prevent truncated or half-tagged outputs.
- **Native metadata writes over FFmpeg subprocesses.** This removes process startup and transcoding risks. Unsupported tag containers degrade conservatively.
- **Metadata enrichment is evidence-driven.** Correctness is defined as preserving existing data unless a container identifier or high-confidence unique match justifies change; network availability is never assumed.
- **Initial scan precedes watch.** A watch invocation first converges existing content, then processes changes through the identical pipeline.

## 13. Things That Are Intentionally Not Documented Here

This file is not the full CLI reference, format-by-format cipher specification, provider API schema, release changelog, or user tutorial. Those belong in `README.md`, source-level adapter documentation, and future provider notes. Implementation sequencing and acceptance commands live in `docs/plans/`.

## 14. Maintenance Notes

Update this document when entry points, major directories, job lifecycle, output safety rules, format/key/provider seams, metadata precedence, or concurrency ownership changes. Small flag additions and individual suffix additions do not require an architecture rewrite unless they alter an invariant.
