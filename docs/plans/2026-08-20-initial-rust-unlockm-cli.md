# Initial Rust `unlockm` CLI Implementation

## Status

Done

Created: 2026-08-20
Last Updated: 2026-08-20

Last updated: 2026-08-20 — independent review fixes implemented and fully locally validated; pending re-review.

This plan covers the first production implementation in the currently empty `unlockm` repository. Update this file continuously during implementation according to `docs/PLANS.md`.

## Objective

Build a production-quality Rust CLI named `unlockm` that converts encrypted music downloads into standard audio files. It must accept a file or recursive directory, preserve nested layout by default, copy non-encrypted companion files by default, support optional flat/no-copy modes, continuously process changes in watch mode, and conservatively update metadata and album art when requested.

The technical outcome is a cross-platform, high-throughput, bounded-memory pipeline that reuses the author's Rust cipher crates, processes independent files in parallel, isolates per-file failures, and publishes outputs atomically.

Success is observable when a user can:

- Run `unlockm -o <output-dir> <file-or-dir>` and receive valid standard audio plus mirrored companion files.
- Add `--flat` to place outputs directly under one directory, with safe collision errors.
- Add `--no-copy` to omit passthrough files without suppressing audio conversion.
- Add `--watch` to process the initial tree and later recursive changes with the same behavior.
- Add `--update-metadata` to preserve existing tags and enrich only from verified NCM/QMC identity data, including correct cover art, without audio transcoding.
- Control CPU/I/O pressure with `--jobs` and `--buffer-size` and observe parallel speedup on suitable multi-file workloads.

## Scope

### User-facing CLI

- One positional `<INPUT>` accepting a regular file or directory.
- `-o, --output <DIR>`; if omitted, use the input directory or a file input's parent (in-place compatibility behavior).
- `--flat`, `--no-copy`, `--overwrite`, `--watch`, `--update-metadata`.
- `--jobs <N>` defaulting to available parallelism and `--buffer-size <BYTES>` defaulting to 1 MiB, subject to benchmark confirmation.
- `--metadata-jobs <N>` to independently bound network operations.
- `--qmc-mmkv`, `--qmc-mmkv-key`, `--kgg-db`, and a documented explicit key-map input for files whose ekey is not embedded.
- `--log-level`, `--json-logs`, and `--supported-ext`; warn-level human logs by default and an unconditional final batch summary.
- Exit 0 for a successful batch or graceful watch shutdown, 1 for fatal initialization/runtime failure, Clap's normal usage code for argument errors, and 3 when a batch completes with file conflicts/failures.

### File and directory behavior

- Recursive scan without following directory symlinks. Symlinks are skipped with a warning in v1.
- Mirror relative paths and empty directories by default.
- Copy unknown/non-encrypted files by default, preserving useful permissions and modification time where supported.
- In flat mode, apply flattening to both converted and copied files; fail all same-run claimants of a colliding destination rather than silently selecting one.
- Skip an existing destination unless `--overwrite` is set. `--overwrite` does not resolve two-source flat collisions.
- Treat passthrough where source and destination are the same file as a successful no-op.
- Prune a proper output subtree inside the input from scans and watch events.
- Keep sources; source deletion is not part of v1.
- Write to same-directory temporary files and publish only complete results.

### Initial format parity

Match the Go reference at `d0d3647...`, using longest suffix match plus header validation:

- Netease: `.ncm`.
- QQ Music/QMC: `.qmc0`, `.qmc2`, `.qmc3`, `.qmc4`, `.qmc6`, `.qmc8`, `.qmcflac`, `.qmcogg`, `.tkm`, `.mgg*`, `.mflac*`, Moo Music `bkc*`, Weiyun hex suffixes, and `.mmp4` as listed in `../cli/algo/qmc/qmc.go`.
- Kugou/Viper: `.kgg`, `.kgm`, `.kgma`, `.vpr`, `.kgm.flac`, `.vpr.flac`, including KGM v5 ekey lookup.
- Kuwo: `.kwm`, including positive detection of already-standard content.
- Xiami: `.xm` and its validated encrypted-standard-suffix variants.
- Ximalaya: `.x2m`, `.x3m`, and the format's `.xm` variant.
- QQ Music iOS TM headers: `.tm0`, `.tm2`, `.tm3`, `.tm6`.
- Standard passthrough detection for MP3, FLAC, Ogg, M4A/MP4, WAV, WMA, AAC, and other types exposed by `um_audio` where safe.

Use `umc_*` and `um_audio` from `../lib_um_crypto_rust` as the design reference and pin committed dependencies to revision `5dad838c33c3e4aab198d995160d3fbb9968470f`. A local Cargo patch may be documented for development, but the build must not require sibling checkout paths.

### Metadata behavior

- Preserve decrypted embedded audio tags and pictures by default.
- Parse NCM container JSON and embedded cover locally; download the container's cover URL only when embedded art is absent.
- Parse QMC footer identity. Prefer resource-ID/media-MID track lookup. Use search only when identity lookup is unavailable and a unique score passes strict title/artist/duration gates.
- Merge values field by field with provenance. Never replace a non-empty tag with filename-only data.
- Cover precedence: existing embedded audio picture, container picture, exact-ID network cover, then a strictly verified search cover.
- Validate image magic/MIME, decodeability, response size, and timeouts before embedding.
- Write tags and art natively in Rust without re-encoding audio. Unsupported containers retain original tags and produce a warning rather than failing decryption.
- Cache provider responses and coalesce identical in-flight lookups; rate-limit network concurrency independently of file jobs.

### Quality, compatibility, and release

- Rust 2021 or newer stable edition chosen during bootstrap and an MSRV explicitly recorded in `README.md`/`Cargo.toml`.
- Windows, macOS, and Linux builds; platform-specific automatic key discovery is feature/cfg isolated.
- Unit, integration, watch, fixture, network-mock, and performance tests.
- MIT licensing for newly written project code, with required notices for MIT and MIT/Apache-2.0 reference-derived portions.

## Non-Goals

- Audio transcoding, normalization, format conversion beyond decrypting to the payload's native format, or an FFmpeg runtime dependency.
- GUI, web server, music library organization, playback, lyric conversion, or encrypted QRC support.
- Automatic deletion/move of source files.
- Following directory symlinks or preserving every platform-specific ACL/xattr in v1.
- Guaranteed metadata coverage when a container has no stable identity and the provider match is ambiguous or offline.
- Supporting the Rust reference's additional JOOX, Migu 3D, and Qingting FM adapters in the parity milestone. Their architecture seam is retained for later work.
- Inventing filenames to resolve flat collisions.
- Publishing/installing packages or creating external releases as part of implementation unless separately requested.

## Current Context

### Target repository

- `unlockm` has no Rust source or commits yet.
- `ARCHITECTURE.md` defines the target module boundaries and invariants.
- `AGENTS.md` points architecture-sensitive work to that document.
- `docs/PLANS.md` defines the persistent plan workflow.
- Rust `1.97.1` and Cargo `1.97.1` are available in the current environment; the implementation must still choose and document a realistic MSRV rather than implicitly requiring this local version.

### Go behavioral reference

- `../cli/cmd/um/main.go` parses the existing CLI and contains scan, watch, decrypt, passthrough, and metadata flow.
- Commit `d0d3647d615c8624abf27e30ade07d23da757041` adds warn-default logs, nested mirroring, passthrough copies, failure isolation, and tests in `../cli/cmd/um/main_test.go`.
- `../cli/algo/common/dispatch.go` uses suffix candidate registration followed by decoder validation and handles multi-dot suffixes.
- `../cli/algo/qmc/qmc_meta.go` and `../cli/algo/qmc/client/` implement QMC embedded probing, provider lookup/search, and cover download.
- `../cli/algo/ncm/ncm.go` exposes NCM JSON/cover data and a network cover fallback.
- `../cli/internal/ffmpeg/` demonstrates intended metadata fields and cover rules, but this rewrite replaces subprocess-based writing with a native tag adapter.
- `../cli/algo/kgm/pc_kugou_db/cipher_windows.go` documents KGM v5 database decryption/query behavior.
- `../cli/internal/mmkv/` and `../cli/algo/qmc/qmmac/` document QMC key-map loading and macOS discovery.

Known reference issues must not be copied: the watch is not recursive, exclusive-open does not reliably mean a file is stable cross-platform, batch work is serial, output writes are non-atomic, QMC search takes the first result, and metadata code contains field/cover-ID edge cases.

### Rust crypto reference

- `../lib_um_crypto_rust/um_cli/src/buffered_decrypt.rs` demonstrates offset-aware streaming with a 4 MiB buffer.
- `../lib_um_crypto_rust/um_cli/src/cmd/` demonstrates setup for NCM, QMC v1/v2, KGM, Xiami, and Ximalaya ciphers.
- `../lib_um_crypto_rust/um_crypto/*` exposes the cipher/header/footer crates, including Kuwo and KGM v5 hooks.
- `../lib_um_crypto_rust/um_audio` detects decrypted payload types and handles leading metadata.
- The Rust reference is tagged `v0.1.12`; its committed revision is the dependency pin for the first implementation.
- `../samples/Claire Rosinkranz - Frankenstein.ncm` is available for a local, non-redistributed end-to-end smoke test.

## Design and Decisions

### Module shape

Use one Cargo package with both `src/lib.rs` and `src/main.rs`. Planned modules are `cli`, `config`, `app`, `pathing`, `formats`, `keys`, `metadata`, `io`, and `error`. A workspace split is deferred until another consumer needs independently versioned crates.

### Job model

`RunConfig` is normalized once. Scans and watcher events create `JobSpec` values. A pure `PathPlanner` computes candidate destinations, and an `OutputReservation` map prevents concurrent publishers from claiming one path. Each job then classifies, streams, optionally enriches tags, validates, and atomically publishes.

Tokio coordinates a multi-thread runtime, signal cancellation, filesystem notifications, and HTTP. `spawn_blocking` runs synchronous file/cipher/tag operations behind a semaphore. `--jobs` limits file work; `--metadata-jobs` limits network calls. A per-file chunk buffer prevents whole-payload allocation.

### Format adapter contract

Each adapter owns suffixes, positive validation, payload bounds, output stem hints, cipher construction, and identity evidence. Registry lookup is case-insensitive and longest-suffix-first. The adapter receives a seekable file and key resolver, then returns a prepared streaming operation. It does not choose final paths or open final destinations.

An encrypted suffix plus failed validation is a conversion error unless positive standard-audio probing proves the source is already playable. Unknown extensions follow copy policy. This avoids misreporting corrupt encrypted downloads as successfully copied files.

### Destination planning

For a directory input, the relative path is calculated from that root. For a file input, the base is its parent. Mirror mode preserves all relative parent components; flat mode drops them. The planner strips the matched encrypted suffix only after adapter validation, then applies the sniffed payload extension.

Because the final extension may be unknown until a decrypted header exists, a job first reserves a source-level work identity and later atomically reserves the resolved destination before publication. Same-run flat collisions mark every claimant conflicted. Existing-output policy is evaluated only after the final path is known.

### Watch state

Watch mode runs an initial scan before subscribing results are considered converged. Use recursive `notify` events with debouncing. A path state table tracks `queued`, `running`, and `dirty`; changes while running set `dirty`, causing one follow-up job. Readiness requires stable length and modification time across two checks, with a bounded retry window. Directory creation/rename triggers subtree enumeration. Removes cancel queued work but never remove outputs.

### Metadata identity and merge

`MetadataValue<T>` records value, provenance, and confidence. NCM container data and exact QQ identity requests are authoritative for textual fields. Existing embedded tags fill gaps. Filename parsing produces queries but never directly produces committed fields.

When QMC search is required, normalize Unicode/case/punctuation, require exact normalized title and at least one artist, compare duration within a documented tolerance, and require a meaningful score margin from the next candidate. Otherwise retain the output unchanged and warn. Provider failures never fail conversion.

Use a native tag library behind an internal `TagEditor` trait. Perform tag writes on the temporary decrypted file. Verify that the resulting file remains probeable and that the chosen tags/picture round-trip before publishing.

### Security and safety

Canonical/lexical containment checks prevent output escape. Hidden owned temp files are the only artifacts cleaned automatically. Logs redact keys, database values, and provider payloads that may contain credentials. HTTP clients use HTTPS, total/connect timeouts, redirect limits, and maximum body sizes. No source is deleted.

## Milestones

### M1. Bootstrap, contracts, and safe path planning

Goal: create a buildable CLI/library skeleton with validated configuration, diagnostics, pure path planning, atomic output primitives, and repository/release basics.

Dependencies: none.

Acceptance criteria:

- `unlockm --help`, `--version`, invalid-argument handling, and supported exit-code plumbing work.
- Mirror/flat, file/directory base paths, proper child-output pruning, same-file no-op, traversal containment, and collision semantics have unit tests.
- Atomic temp publication never truncates a pre-existing output on injected failure.
- No cipher or production conversion is required yet.

Validation:

- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy --all-targets --all-features -- -D warnings`
- `rtk cargo test --lib`
- `rtk cargo run -- --help`

### M2. Parallel recursive batch pipeline and passthrough behavior

Goal: implement shared job scheduling, recursive scans, mirror/flat layouts, copy/no-copy modes, summaries, and failure isolation before introducing format complexity.

Dependencies: M1.

Acceptance criteria:

- Nested companion files and empty directories mirror by default to a separate output root.
- `--no-copy` and `--flat` have end-to-end tests, including collisions.
- Independent jobs run concurrently but do not exceed `--jobs`; memory scales with configured buffers rather than total input size.
- One unreadable or injected-failure file does not block siblings, and the final summary/exit code reports partial failure.
- Output-inside-input scans do not recurse into output.

Validation:

- `rtk cargo test --test batch_layout`
- `rtk cargo test --test failure_isolation`
- `rtk cargo test --test cli_exit_codes`
- `rtk cargo test --all-targets`

### M3. Go-parity format adapters and key sources

Goal: add all in-scope NCM/QMC/KGM/KWM/Xiami/Ximalaya/TM adapters, standard-audio probing, key resolution, and standard-extension output selection.

Dependencies: M2 and the pinned `umc_*`/`um_audio` revision.

Acceptance criteria:

- Every suffix family in Scope appears in `--supported-ext` and has registry precedence tests.
- Each adapter has valid, invalid, truncated-header, and output-hash fixture coverage where a redistributable fixture is available.
- Decryption is chunked and offset-correct across buffer boundaries, including tiny test buffers.
- QMC resolves embedded footer keys and configured MMKV/key-map sources; macOS discovery is cfg-isolated and failures degrade to actionable per-file errors.
- KGM v5 can decrypt/query an explicit database on all supported systems and discover the conventional path on Windows.
- Standard audio with a misleading registered suffix is passed through only after positive probing; corrupt encrypted candidates fail.
- The local NCM sample produces a probeable standard file with an expected stable audio hash, recorded without committing copyrighted output.

Validation:

- `rtk cargo test formats::`
- `rtk cargo test keys::`
- `rtk cargo test --test decrypt_fixtures`
- `rtk cargo run -- --supported-ext`
- `rtk cargo run --release -- -o <temporary-output> '../samples/Claire Rosinkranz - Frankenstein.ncm'`
- Probe the local smoke output with the native detector and, when available, `ffprobe` as an independent check.

### M4. Conservative metadata and album-art enrichment

Goal: implement native tag preservation/writes plus evidence-driven NCM/QMC enrichment with deterministic provider tests.

Dependencies: M3.

Acceptance criteria:

- Conversion without `--update-metadata` performs no network calls and preserves existing embedded tags/pictures.
- NCM metadata and cover precedence work for music and DJ JSON shapes, missing metadata, embedded image, and URL fallback.
- QMC exact-ID lookup is preferred; ambiguous/no-result search leaves tags untouched; a unique verified result writes the intended title, album, artists, and cover.
- Cover validation rejects mislabeled, oversized, invalid, and unsupported images without failing audio conversion.
- MP3, FLAC, M4A/MP4, and Ogg fixtures round-trip supported text fields and pictures without re-encoding audio. Unsupported output containers warn and remain playable.
- Provider timeouts, rate limits, caches, and in-flight coalescing are tested with a local HTTP server; tests never depend on live endpoints.

Validation:

- `rtk cargo test metadata::`
- `rtk cargo test --test metadata_roundtrip`
- `rtk cargo test --test metadata_provider_mock`
- `rtk cargo test --test no_network_by_default`
- Compare pre/post audio payload fingerprints in tag round-trip fixtures.

### M5. Recursive resilient watch mode

Goal: add continuous recursive monitoring on top of the exact batch pipeline.

Dependencies: M2-M4.

Acceptance criteria:

- `--watch` rejects a file input, performs the initial tree conversion, and then handles nested create, modify, and rename-to events.
- Partially written files are not processed until stable; a change during processing produces one follow-up conversion.
- Event storms are coalesced and do not create duplicate concurrent work or repeated metadata calls.
- Newly created directories, separate output roots, proper child outputs, in-place outputs, mirror/flat, copy/no-copy, overwrite, and metadata flags behave like batch mode.
- Per-file errors remain non-fatal; fatal watcher/channel errors are surfaced; Ctrl-C shuts down without publishing partial output.

Validation:

- `rtk cargo test watch::`
- `rtk cargo test --test watch_recursive -- --test-threads=1`
- `rtk cargo test --test watch_stability -- --test-threads=1`
- Manual watch smoke test on macOS plus recorded CI results for Windows and Linux notification backends.

### M6. Performance, cross-platform hardening, and user documentation

Goal: demonstrate the performance objective, close platform gaps, and deliver a usable documented binary.

Dependencies: M1-M5.

Acceptance criteria:

- Release benchmarks cover a large stream and many-file workload at `--jobs 1` and default jobs; results and hardware are recorded in this plan.
- A release single-stream conversion is within 15% of a direct pinned-crypto loop on the same fixture, and multi-file throughput improves over `--jobs 1` when storage is not the bottleneck.
- Peak process memory for synthetic payloads remains approximately `jobs * buffer_size + 128 MiB` or less, excluding OS page cache and explicitly documented provider/key indexes.
- Buffer/job defaults are confirmed or changed based on evidence; no whole-audio temporary memory buffers remain.
- `cargo fmt`, Clippy, all tests, docs, dependency/license review, and optimized builds pass.
- `README.md` documents install/build, MSRV, examples, defaults, overwrite/collision behavior, watch semantics, metadata limits/privacy/network use, key configuration, supported formats, and troubleshooting.
- CI checks Linux, macOS, and Windows; release profile uses evidence-backed optimization settings.

Validation:

- `rtk cargo bench`
- `rtk cargo test --all-targets --all-features`
- `rtk cargo clippy --all-targets --all-features -- -D warnings`
- `rtk cargo fmt --all -- --check`
- `rtk cargo build --release --locked`
- `rtk cargo tree --duplicates`
- Platform CI commands and benchmark/RSS evidence recorded under Completion Evidence.

## Task Breakdown

### M1 tasks

- [x] T1.1 Initialize `Cargo.toml`, lockfile, `src/lib.rs`, and `src/main.rs`; set package metadata, MSRV, profiles, features, and pinned crypto/audio git dependencies; verify `cargo check`.
- [x] T1.2 Add Clap definitions in `src/cli.rs` and validated immutable types in `src/config.rs`; test defaults, invalid values, file/watch rejection, and exit-code mapping.
- [x] T1.3 Add typed errors and `tracing` setup in `src/error.rs` and diagnostics support; test warn default, verbose levels, JSON output, and secret redaction.
- [x] T1.4 Implement pure mirror/flat planning, containment, output-subtree detection, and collision keys in `src/pathing.rs`; exhaustively unit-test platform-neutral path cases.
- [x] T1.5 Implement owned same-directory temp files and atomic publish/skip/overwrite behavior in `src/io/atomic.rs`; use fault injection to verify destination preservation and cleanup.
- [x] T1.6 Add `LICENSE`, third-party attribution scaffolding, `.gitignore`, and minimal build/CI configuration without claiming feature completion.

### M2 tasks

- [x] T2.1 Define `JobSpec`, `JobOutcome`, counters, cancellation, and output reservations in `src/app/`; unit-test state transitions and two-source conflicts.
- [x] T2.2 Implement recursive enumeration and empty-directory mirroring in `src/app/scan.rs`, skipping symlinks and pruning output subtrees; test nested and in-place trees.
- [x] T2.3 Implement the bounded Tokio/`spawn_blocking` runner in `src/app/runner.rs`; test job and open-file limits with instrumentation.
- [x] T2.4 Implement passthrough classification/copy/no-op behavior in `src/app/job.rs`; preserve permissions/mtime where supported and test no-copy/overwrite semantics.
- [x] T2.5 Add batch summaries, partial-failure exit code, and resilience integration tests modeled on `../cli/cmd/um/main_test.go`.
- [x] T2.6 Add flat-mode preflight/reservation conflict tests covering converted/copied name collisions and case-insensitive filesystem considerations.

### M3 tasks

- [x] T3.1 Define `FormatAdapter`, `PreparedDecrypt`, registry, longest-suffix matching, and standard-audio probe in `src/formats/`; add generic chunk-boundary test harnesses.
- [x] T3.2 Add NCM and QMC v1/v2 adapters using pinned `umc_ncm`, `umc_qmc`, and `um_audio`; cover footer trimming, keys, output sniffing, and all QMC suffix aliases.
- [x] T3.3 Add QMC key-map/MMKV readers and cfg-isolated macOS v8/v10 discovery in `src/keys/qmc.rs`; port only required MIT logic with attribution and redacted diagnostics.
- [x] T3.4 Add KGM/KGG/KGMA/VPR adapters and KGM v5 key lookup in `src/formats/kgm.rs` and `src/keys/kgm.rs`; decrypt the database with `umc_kgm` and query a read-only/in-memory SQLite image.
- [x] T3.5 Add KWM v1 (and v2 when an explicit ekey is available), Xiami, Ximalaya, and TM adapters; resolve the shared `.xm` suffix by positive validation order.
- [x] T3.6 Add the complete supported-extension report and fixtures for valid/invalid/truncated/disguised-standard inputs, including multi-dot and uppercase names.
- [x] T3.7 Run and record the local NCM smoke conversion, output type, audio hash, and any unavailable real-world fixtures as explicit gaps.

### M4 tasks

- [x] T4.1 Define metadata/provenance models, Unicode normalization, merge precedence, candidate scoring, and ambiguity thresholds in `src/metadata/model.rs` and `merge.rs`; table-test every precedence conflict.
- [x] T4.2 Implement native tag/picture read-write adapters and round-trip validation in `src/metadata/tags.rs`; real FLAC, M4A/MP4, and Ogg round-trip fixtures remain required (MP3 and unsupported-container coverage completed).
- [x] T4.3 Parse NCM music/DJ metadata and cover sources in `src/metadata/ncm.rs`; test varied artist JSON shapes, missing fields, bad URLs, and cover precedence.
- [x] T4.4 Implement a reusable bounded HTTP client and QQ exact-ID/search/cover provider in `src/metadata/provider/qq.rs`; enforce status, redirect, timeout, and body-size rules.
- [x] T4.5 Add metadata request semaphore, response cache, and in-flight deduplication; verify one request per identity under concurrent batch/watch jobs.
- [x] T4.6 Integrate `--update-metadata` into the temporary-output phase; prove offline/provider/tag failures do not fail conversion or modify an existing destination.
- [x] T4.7 Add deterministic provider-server and audio-payload-integrity integration tests; explicitly verify no network call without the flag.

### M5 tasks

- [x] T5.1 Implement `notify` recursive subscription and normalized event conversion in `src/app/watch.rs`; unit-test create/modify/rename/remove/directory mappings.
- [x] T5.2 Implement debouncing, stable-size/mtime retries, queued/running/dirty coalescing, and bounded expiry; test event storms and changes during processing.
- [x] T5.3 Route initial scans and stabilized events through the existing runner/planner, including output pruning, temp-file filtering, and in-place no-op behavior.
- [x] T5.4 Implement Ctrl-C cancellation and fatal/non-fatal watcher error policy; test that cancellation removes owned temps and leaves sources/destinations intact.
- [x] T5.5 Add serialized recursive watch integration tests and platform-specific CI smoke coverage with diagnostic artifacts on failure.

### M6 tasks

- [x] T6.1 Add Criterion or an equivalent stable benchmark harness for direct cipher, one-file pipeline, and many-file pipeline paths; generate deterministic non-copyrighted data.
- [x] T6.2 Measure release throughput/RSS against direct pinned crypto and `--jobs 1`; tune buffer, admission limits, metadata concurrency, and release profile from evidence.
- [x] T6.3 Audit blocking/async boundaries, open-file limits, allocations, unnecessary copies, lock contention, provider bounds, secret logging, and path race handling; add regressions for findings.
- [x] T6.4 Add Windows/macOS/Linux CI for format, batch, metadata, and watch suites; record platform skips as gaps until they are resolved.
- [x] T6.5 Write `README.md` and reconcile `ARCHITECTURE.md`, CLI help, supported formats, licenses/notices, privacy/network behavior, and troubleshooting with the implemented state.
- [x] T6.6 Run the complete validation matrix, record exact results below, and move the plan to `In Review` rather than `Done` pending independent review.

## Validation Strategy

### Fast checks during implementation

- Run `rtk cargo fmt --all -- --check`, targeted unit tests, and targeted integration tests for each task.
- Run `rtk cargo clippy --all-targets --all-features -- -D warnings` at every milestone boundary.
- Use tiny buffers (1, 7, 16, 4 KiB) in adapter tests to expose offset/boundary errors.
- Inject failures at read, decrypt, tag, flush, and publish phases to prove atomicity and sibling resilience.

### Functional matrix

Test file input and directory input across:

- Mirror versus flat.
- Copy versus no-copy.
- Existing output with and without overwrite.
- Output equal to input, outside input, and a proper child of input.
- Batch versus initial watch scan versus later watch event.
- Metadata disabled, exact local/container metadata, exact network identity, ambiguous search, offline, and invalid cover.
- At least one supported encrypted adapter family plus unknown, standard, corrupt encrypted-looking, and unreadable files.

### Fixture integrity

- Prefer generated or upstream-license-compatible small fixtures with attribution.
- Store expected decrypted SHA-256 values, detected audio types, and relevant metadata in fixture manifests.
- Do not commit the local copyrighted NCM sample or its decrypted output. Record only the command and non-sensitive hash/type result.
- Mock provider endpoints for automated tests. Live QQ/NCM calls are optional manual smoke checks and never gate deterministic CI.

### Performance validation

- Benchmark release binaries on a stated CPU/storage/filesystem with warm-up and multiple samples.
- Compare direct cipher loop, `unlockm --jobs 1`, default jobs, and when feasible the Go reference.
- Separate one-large-file throughput from many-file aggregate throughput; per-file ciphering is sequential by design.
- Measure peak RSS with input size much larger than RAM budgets and confirm no full-payload allocation.
- Record whether storage saturation prevents parallel scaling; do not call that a CPU-pipeline failure without evidence.

### Final commands

- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy --all-targets --all-features -- -D warnings`
- `rtk cargo test --all-targets --all-features`
- `rtk cargo build --release --locked`
- `rtk cargo bench`
- Platform CI on Windows, macOS, and Linux.
- Manual recursive watch smoke with a file copied in chunks, nested directory creation, overwrite change, and Ctrl-C.

## Risks and Mitigations

- **Pinned crypto crates are git-only or their APIs are incomplete.** Pin a revision, wrap every crate behind local adapters, and use a documented local Cargo patch only for development. If a required bug fix is necessary, contribute/fork explicitly and update the decision log rather than copying silently.
- **QMC/KGM keys are absent or platform stores change.** Keep key sources composable, accept explicit paths/maps, report the missing identifier and source attempts without secrets, and test discovery independently of ciphers.
- **Metadata APIs are unofficial and may drift.** Isolate provider models, mock response variants, prefer embedded/exact identity, and make all network enrichment best effort.
- **Native tag writing may not support every detected payload.** Gate by container, retain existing data on unsupported types, verify round trips, and never transcode merely to add tags.
- **Filesystem notifications are lossy/platform-specific.** Perform an initial scan, debounce/stabilize, coalesce state, and keep a future periodic reconciliation seam. Validate each platform in CI/manual smoke.
- **Watch may process its own output.** Prune proper output subtrees, ignore owned temp names, make in-place passthrough a no-op, and deduplicate per canonical path.
- **Flat paths race or overwrite unrelated files.** Reserve resolved destinations, reject same-run collisions, and distinguish collision from existing-output overwrite.
- **Atomic replacement differs on Windows.** Encapsulate publish behavior and test skip/replace/failure on every platform; never fall back to truncating the destination before a complete temp exists.
- **Parallelism can degrade HDD/network-volume performance or exhaust descriptors.** Bound jobs/open files, expose tuning, keep network concurrency separate, and choose defaults from benchmarks.
- **Large buffers multiplied by jobs inflate RSS.** Treat `jobs * buffer_size` as an explicit memory budget and benchmark 1 MiB versus 4 MiB before finalizing defaults.
- **Reference behavior contains correctness bugs.** Treat Go code as behavioral evidence, not a specification; preserve its successful nested/resilient semantics while adding explicit tests for known watch/metadata/output weaknesses.
- **Fixture/licensing constraints limit format proof.** Record unavailable real fixtures as gaps, use upstream licensed vectors and synthetic data, and do not imply untested format coverage is validated.

## Progress

- [x] 2026-08-20: Inspected Go CLI commit `d0d3647...`, including batch resilience, passthrough, watch, QMC/NCM metadata, key sources, and tests.
- [x] 2026-08-20: Inspected Rust crypto/audio revision `5dad838...` (`v0.1.12`) and its example CLI adapters.
- [x] 2026-08-20: Created `ARCHITECTURE.md`, `AGENTS.md`, `docs/PLANS.md`, and this Ready implementation plan.
- [x] 2026-08-20: Completed T1.1: created the locked Rust 2021 package, stable MSRV declaration, thin executable/library split, release profile, and revision-pinned `um_audio`/`umc_*` dependencies. `rtk cargo check` passed.
- [x] 2026-08-20: Completed T1.2: Clap models all baseline flags; configuration rejects zero limits and file watch inputs, and unit/integration checks cover default budgets, usage failures, normal/partial batch exit codes, and input-less `--supported-ext`.
- [x] 2026-08-20: Completed T1.3 and T1.5: real binary tests verify warn and JSON event rendering plus accepted debug verbosity, while diagnostics redact secrets. Atomic tests cover skip, overwrite, drop cleanup, and injected abort preserving existing output and removing only the owned temp.
- [x] 2026-08-20: Completed T2.1/T2.3: added shared cancellation ownership, state outcome types, reservation conflict tests, and runner metrics. Deterministic real-runner test copies six 4 MiB inputs with `jobs=2`, observes concurrent work, proves both peak counters are bounded at two, and confirms no active job remains.
- [x] 2026-08-20: Completed T2.2/T2.4/T2.5: recursive scans mirror empty directories, prune child outputs, and skip symlinks (Unix coverage). Passthrough supports no-copy, overwrite, same-file no-op, permissions, and mtime preservation. Integration suites verify summaries, partial exit 3, and sibling resilience.
- [x] 2026-08-20: Completed T1.4: added shared mirror/flat destination planning, multi-dot encrypted suffix stripping, output containment, and reservation collision tests.
- [x] 2026-08-20: Completed T1.6: added MIT license, third-party notice scaffold, target ignore rules, README status disclosure, and Linux/macOS/Windows CI workflow.
- [x] 2026-08-20: Completed T2.6 and M2: every batch input now completes its bounded encrypted preflight into an owned hidden stage before grouping by the platform-aware final-destination collision key. A same-run collision marks every converted/copied claimant conflicted, including with `--overwrite`; staged artifacts are dropped before publication. `rtk cargo test --test batch_layout`, `rtk cargo test --test failure_isolation`, `rtk cargo test --test cli_exit_codes`, and `rtk cargo test --all-targets` passed.
- [x] 2026-08-20: Completed T3.1: `src/formats/{adapter,registry,audio}.rs` now provides a used `FormatAdapter` registry and `PreparedDecrypt` streaming contract. Preflight uses registry-selected prepared operations, considers all case-insensitive matching suffixes longest-first, requires adapter validation before treating a registered candidate as encrypted, and checks standard audio first. Tests cover registry aliases, invalid/truncated candidates, disguised standard audio, shared `.xm` ordering via registry order, and transform offsets with 1/7/16/4096-byte buffers.
- [x] 2026-08-20: Completed M3/T3.2-T3.7. QMC v2 is streamed with parsed footer trimming and ekey-only cipher construction; a self-contained 720 B MIT/Apache upstream PC footer fixture decrypts to deterministic FLAC across 1/7/16/4096-byte buffers. The NCM 1056 B upstream parser vector verifies header, offset, metadata hint, and decrypted ID3 fragment. QMC explicit maps/plain MMKV, cfg-isolated macOS discovery, KGM v5 read-only SQLite lookup, all remaining adapters, and a deduplicated supported-extension report are implemented. No raw-key fallback exists.
- [x] 2026-08-20: Completed M4/T4.1-T4.7. Metadata has field-level provenance and Unicode-normalized ambiguity gates; NCM music/DJ JSON and embedded-cover precedence; Lofty native tag/picture writes with post-write re-read; and a bounded no-redirect QQ client with cache, request permits, and in-flight coalescing. Metadata runs only on the staged temporary output under `--update-metadata`; all enrichment failures are warnings and preserve conversion success.
- [x] 2026-08-20: Closed T4.2 review gap with self-contained base64 Lofty FLAC, Ogg, and AAC/M4A fixtures. Each fixture decodes only into a temporary test file; the integration test writes title/album/artist plus a valid cover, re-reads every field/picture, compares decoder-derived audio properties, and verifies a container-aware FLAC/mdat/Ogg-packet audio payload SHA-256 is unchanged. Attribution and source URLs are in `THIRD_PARTY_NOTICES.md`.
- [x] 2026-08-20: Completed M5/T5.1-T5.5. Recursive `notify` events are normalized (including rename-from/to/both and directory removes), debounced, stability-gated by two matching size/mtime samples, and expire after five seconds. The watcher shares one bounded runner resource set with batch work; queued/running/dirty state coalesces changes and schedules exactly one follow-up. Per-path cancellation suppresses atomic publication on remove/Ctrl-C, owned temps and child outputs are filtered, and a one-second reconciliation closes backend loss/subscribe races through the same scanner/planner/runner path. Serialized integration tests wait for published output observables rather than fixed delivery sleeps.
- [x] 2026-08-20: Completed T6.3-T6.5. Audit found overwrite publication removed a pre-existing destination before a Windows-incompatible rename; `AtomicOutput` now moves it to an owned backup and restores it if publish fails. The atomic regression also checks backup cleanup. CI's existing Windows/macOS/Linux matrix now runs the real notify suites serially after the common matrix. README and architecture document stability, cancellation, reconciliation, output safety, privacy/network semantics, supported formats, MSRV, key configuration, and troubleshooting.
- [x] 2026-08-20: Completed T6.1-T6.2. Added `benches/throughput.rs` as a stable `[[bench]] harness = false` executable. It streams deterministic non-copyrighted QMCv1-encrypted FLAC-looking data, checks each published SHA-256, and reports direct pinned-cipher, one-file pipeline, and many-file `jobs=1`/default rates. Staged decrypt publication now renames its complete same-filesystem owned stage into place instead of copying the entire audio a second time; overwrite retains the recoverable backup behavior.
- [x] 2026-08-20: Completed T6.6. Local final matrix is green: format, batch, metadata, watch, and failure suites are included in the all-target test run; formatting, warnings-denied Clippy, locked release build, duplicate-dependency inspection, and release benchmark all completed. CI is configured to execute the same matrix on Linux, macOS, and Windows, with watch suites serialized. The plan is intentionally `In Review`, not `Done`, pending independent review and remote CI evidence.
- [x] 2026-08-20: Resolved the five independent review findings pending re-review. QMC footer identities now reach staged metadata enrichment; exact and strict-search provider flows, validated covers, cache, and no-network behavior have deterministic loopback coverage. MMKV is compatible with the stated `go-mmkv` v0.1.4 plain/encrypted vectors and macOS v8/v10 derivation. Watch reconciliation now uses a normalized fingerprint snapshot and has a multi-period quiescence regression. Cover precedence is field-specific, and atomic publish has a final-rename failure/restore regression.

## Discoveries

- The requested Go commit is the current local Go reference HEAD and is one commit ahead of its configured `origin/main`.
- The Go commit already establishes desirable defaults for warn-level logging, nested mirroring, passthrough copies, complete-output skipping, and continue-after-error semantics.
- The Go watch implementation watches only the root directory, sleeps in its event loop, and uses an exclusive-open heuristic that is not portable; recursive/stability semantics need a fresh design.
- Go metadata update materializes full decrypted QMC audio and shells out to FFmpeg. The Rust rewrite can avoid both costs with a temporary streaming output plus native tags.
- Go QMC search selects its first result, and its metadata/cover code has suspicious field conditions; conservative scored matching is required to meet correctness goals.
- The Rust workspace provides ciphers and audio detection but not the requested directory/watch/application layer, complete key discovery, or tag writing.
- Rust KGM v5 accepts an ekey and exposes database decryption, so `unlockm` must add the read-only SQLite key-index layer.
- The `.xm` suffix is shared by Xiami and Ximalaya and therefore requires ordered positive validation, not extension-only dispatch.
- On macOS, temporary paths may appear under `/var` before canonicalization but inputs resolve under `/private/var`. Output normalization must happen before output-child pruning; otherwise mirroring recursively creates `out/out/...` until filename limits are reached. `RunConfig` now normalizes an existing output or its existing parent.
- `um_audio` treats short header buffers conservatively, so standard-audio recognition must keep a sufficiently large probe and never use it as sole evidence for a corrupt registered encrypted suffix.
- Initial watcher implementation now coalesces events pending through a 250 ms debounce window before handing paths to the runner. It still lacks the required stable-size/mtime and running/dirty state table, so no M5 task is checked.
- Before T3.4, the pinned KGM crate exposed v5 through `Decipher::new_v5` but its SQLite database lookup remained an application concern. The completed implementation keeps its ekey lookup hash-addressed, avoiding an unsafe arbitrary-key fallback.
- Flat collision detection cannot use only encrypted suffixes: QMCv1 payloads in particular require a completed bounded decrypt and native audio probe to know the final extension. Retaining the private staged output avoids a second decryption and gives every claimant a deterministic cleanup path.
- Local NCM smoke source is available outside the repository and must remain uncommitted. On this macOS runner the release binary converted it to a probeable FLAC; its type/hash are now recorded in M3 completion evidence.
- The upstream NCM parser vector intentionally has only a 15-byte encrypted audio fragment. It is used for container/header/metadata assertions, not claimed as a probeable-media conversion fixture; the separate local smoke proves a real probeable FLAC without redistributing it.
- The QMC MMKV reader intentionally accepts only plaintext string-entry stores. A password-bearing/encrypted store fails closed with an actionable export-to-key-map diagnostic, rather than interpreting bytes as a key.
- Metadata integration creates/drops Reqwest clients within the bounded blocking worker rather than Tokio's async scheduler. This avoids the runtime-drop panic observed in runner tests while keeping network work outside the runtime.
- Lofty's overall bitrate includes container/tag overhead and changes after a valid metadata write. T4.2 therefore compares duration, audio bitrate, sample rate, bit depth, channels, and a container-aware audio-payload hash rather than treating total file/container bitrate as audio mutation.
- macOS can report FSEvents paths through `/var` while configuration canonicalization uses `/private/var`; watch event paths must canonicalize the existing path (or existing parent for a removed path) before containment checks. A low-frequency reconciliation is retained as a notification-loss safety net and is intentionally routed through the identical scan/runner path.
- The watcher snapshot must be taken before its subscription-ready signal. A snapshot taken after the subscription can absorb a file created during setup and make it appear already converged; snapshot-before-subscribe plus reconciliation makes that race observable and recoverable.
- The workspace sandbox forbids loopback binds. Provider integration is deterministic but must run through the approved elevated path; this is an environment limitation rather than a network dependency.
- `MetadataClient` had a generic URL dispatcher shared by trusted QQ RPC and untrusted cover metadata. The allowlist must be checked before cache/in-flight coordination and before client dispatch, otherwise an untrusted source can reach private network origins.

## Decision Log

- **2026-08-20 — Ready without additional user questions.** Platform, output, collision, metadata, and concurrency details are documented as explicit assumptions that preserve reference compatibility and safety; none blocks implementation.
- **2026-08-20 — Cross-platform target.** Windows, macOS, and Linux are first-release targets; discovery integrations may be cfg-specific but explicit key sources remain portable.
- **2026-08-20 — In-place compatibility default.** Omitted output means source directory/file parent; a proper child output is pruned and same-path passthrough is a no-op.
- **2026-08-20 — Mirror/copy defaults.** `--flat` and `--no-copy` are opt-outs. Flat conflicts fail instead of overwrite or rename.
- **2026-08-20 — Reuse pinned Rust crypto.** Depend on revision `5dad838...` through git and local adapters; do not commit sibling path dependencies.
- **2026-08-20 — Hybrid concurrency.** Tokio coordinates; bounded blocking jobs run cipher/file/tag work; metadata has a separate limit.
- **2026-08-20 — Native, conservative metadata.** No FFmpeg runtime; exact/container identity may enrich, ambiguous search never mutates tags, and network failure never fails conversion.
- **2026-08-20 — Atomic outputs and retained sources.** All converted/copied artifacts publish from owned temps, and v1 has no source-removal feature.
- **2026-08-20 — Go parity before extra Rust formats.** JOOX, Migu 3D, and Qingting FM remain documented extension work rather than expanding the initial delivery.
- **2026-08-20 — Keep corrupt or keyless registered formats fail-closed.** Registry selection never copies ciphertext as a successful passthrough; missing QMC/KGM keys remain actionable per-file conversion errors.
- **2026-08-20 — M3 key-source safety.** QMC adapters construct `QMCv2Cipher` only from an embedded/configured ekey, whose decode stays inside the pinned crypto crate; no CLI or database value is treated as a raw cipher key. KGM v5 accepts only an ekey found by its audio hash in the decrypted SQLite key index.
- **2026-08-20 — Metadata is post-decrypt best effort.** Only the staged temporary audio is considered for tag mutation. Provider/cover/tag errors warn and leave the conversion publish path intact; no request is made unless `--update-metadata` is enabled.
- **2026-08-20 — Watch resource lifetime.** Watch jobs share the batch semaphore, key sources, metadata client/cache, and reservations, so an event storm cannot multiply `--jobs` or repeated provider lookups. Cancellation is cooperative at chunk boundaries and always aborts an owned temp before it can publish.
- **2026-08-20 — Portable overwrite publication.** Existing outputs are moved to an owned same-directory backup before the final temp rename and restored if that rename fails. This avoids a delete-before-publish data-loss window on platforms whose rename cannot replace an existing destination.
- **2026-08-20 — Benchmark defaults retained.** The 1 MiB buffer and available-parallelism job default remain evidence-backed: the release one-file pipeline stays within 15% of the direct QMCv1 loop and many independent files scale on local fast storage. The one-file result is intentionally sequential because a cipher payload is never split across workers.
- **2026-08-20 — QMC enrichment identity gate.** Parsed QTag/STag/PCv2 footer identity uses the exact provider endpoint. Strict title/artist/duration search is permitted only when no footer identity exists; an exact lookup failure never downgrades into fuzzy matching.
- **2026-08-20 — MMKV compatibility boundary.** The Rust port follows `go-mmkv` v0.1.4's little-endian payload length, leading varint, string containers, `.crc` IV offset, zero-padded/truncated 16-byte password, and AES-128-CFB behavior. MMKV is still only an ekey source, never a raw cipher-key fallback.
- **2026-08-20 — Reconciliation fingerprint ownership.** The known snapshot records source fingerprints at successful scheduling. Event/reconcile changes coalesce into one dirty follow-up, while a static tree produces no repeated work.
- **2026-08-20 — Metadata outbound-origin policy.** Production provider RPC accepts only `https://u.y.qq.com`; cover downloads accept HTTPS without userinfo only from `y.gtimg.cn`, `imgcache.qq.com`, and `music.126.net` subdomains. Test loopback is a `cfg(test)` crate-private seam and cannot configure production transport.

## Completion Evidence

Current implementation evidence:

- 2026-08-20: `rtk cargo check` passed after resolving the pinned git packages.
- 2026-08-20: `rtk cargo test --lib` passed: 10 tests.
- 2026-08-20: `rtk cargo test --all-targets` passed: 16 tests in 5 suites, including recursive layout, empty-directory mirroring, no-copy, flat collision, and per-file failure-isolation coverage.
- 2026-08-20: after format-adapter and configuration additions, `rtk cargo test --all-targets` passed: 20 tests in 5 suites; `rtk cargo clippy --all-targets --all-features -- -D warnings` passed.
- 2026-08-20: `rtk cargo fmt --all`, `rtk cargo clippy --all-targets --all-features -- -D warnings`, and `rtk cargo build --release --locked` passed (release build: 96 crates).
- 2026-08-20: `rtk cargo run -- --supported-ext` passed after correcting the conditional positional argument; the report no longer requires a dummy input path.
- 2026-08-20: M1 validation passed: `rtk cargo fmt --all -- --check`, `rtk cargo clippy --all-targets --all-features -- -D warnings`, `rtk cargo test --lib` (18 passed), and `rtk cargo run -- --help`.
- 2026-08-20: M2 validation passed: `rtk cargo fmt --all -- --check`; `rtk cargo test --test batch_layout` (6 passed); `rtk cargo test --test failure_isolation` (1 passed); `rtk cargo test --test cli_exit_codes` (2 passed); `rtk cargo test --all-targets` (35 passed across 6 suites); and `rtk cargo clippy --all-targets --all-features -- -D warnings` (passed).
- 2026-08-20: local M3 NCM smoke (non-redistributed input): `rtk cargo run --release -- -o /private/tmp/unlockm-m3-smoke '../samples/Claire Rosinkranz - Frankenstein.ncm'` completed with `converted=1`. Native output was `Claire Rosinkranz - Frankenstein.flac`; `rtk file` identified a 16-bit stereo 44.1 kHz FLAC stream and SHA-256 was `a0fd78ef14808cb8d78e6706a385adc834623e6692428f6ac4041b14c0077429`.
- 2026-08-20: T3.1 validation passed: `rtk cargo fmt --all -- --check`; `rtk cargo test formats::` (5 passed); and `rtk cargo clippy --all-targets --all-features -- -D warnings`.
- 2026-08-20: M3 validation passed: `rtk cargo fmt --all -- --check`; `rtk cargo test formats::` (9 passed); `rtk cargo test keys::` (2 passed); `rtk cargo test --all-targets` (48 passed across 7 suites); `rtk cargo clippy --all-targets --all-features -- -D warnings`; `rtk cargo run -- --supported-ext`; and `rtk cargo build --release --locked`.
- 2026-08-20: final local M3 NCM smoke: `rtk cargo run --release -- -o /private/tmp/unlockm-m3-smoke-final '../samples/Claire Rosinkranz - Frankenstein.ncm'` completed with `converted=1`. `rtk file` reported 16-bit stereo 44.1 kHz FLAC audio; SHA-256 was `a0fd78ef14808cb8d78e6706a385adc834623e6692428f6ac4041b14c0077429`. The input and temporary output remain outside the repository.
- 2026-08-20: M4 validation passed: `rtk cargo test metadata::` (4 passed); `rtk cargo test --test metadata_roundtrip` (2 passed, including real silent MP3/ID3 payload preservation); `rtk cargo test --test no_network_by_default` (1 passed); `rtk cargo test --all-targets` (54 passed across 9 suites); and `rtk cargo fmt --all -- --check`.
- 2026-08-20: M5 validation passed: `rtk cargo test watch::` (2 passed); `rtk cargo test --test watch_recursive -- --test-threads=1` (2 passed); `rtk cargo test --test watch_stability -- --test-threads=1` (1 passed); `rtk cargo clippy --all-targets --all-features -- -D warnings` (passed). The recursive integration suite subscribes before creating/renaming/modifying files and waits on resulting output bytes; it also proves owned temp filtering and shutdown cleanup.
- 2026-08-20: T6.3-T6.5 validation passed: `rtk cargo fmt --all -- --check`; `rtk cargo test io::atomic::tests::` (4 passed); and `rtk cargo clippy --all-targets --all-features -- -D warnings` (passed). `rtk cargo test --all-targets --all-features` passed outside the filesystem sandbox (59 tests); the sandbox itself forbids the deterministic loopback metadata mock's bind and is not a project failure.
- 2026-08-20: T6.1-T6.2 benchmark evidence, local macOS: Apple M3, default 8 jobs, 1 MiB buffer, 64 MiB deterministic QMCv1 stream split into 8 independent many-file inputs. `rtk cargo bench --bench throughput` reported direct QMCv1 1207.60 MiB/s; one-file full pipeline at jobs=1 1027.43 MiB/s (85.1% of direct, 14.9% overhead); many-file jobs=1 354.10 MiB/s; many-file default jobs 3489.81 MiB/s (9.86x scaling). `rtk proxy /usr/bin/time -l cargo bench --bench throughput` outside the sandbox reported 88,834,048 B maximum RSS (84.7 MiB), below the `8 * 1 MiB + 128 MiB = 136 MiB` target; macOS also reported 63,668,848 B peak memory footprint. The many-file scaling is page-cache/local-storage sensitive and is evidence for a non-I/O-bound local run, not a portability promise.
- 2026-08-20: T4.2 validation passed: `rtk cargo test --test metadata_roundtrip` (3 passed). The fixtures are upstream Lofty minimal vectors under MIT OR Apache-2.0, stored as repository text rather than runtime/downloader dependencies.
- 2026-08-20: T6.6 final local validation passed: `rtk cargo fmt --all -- --check`; `rtk cargo clippy --all-targets --all-features -- -D warnings`; `rtk cargo test --all-targets --all-features` (elevated local-loopback run; 60 tests after the new metadata fixture case); `rtk cargo build --release --locked`; `rtk cargo tree --duplicates` (duplicate transitive versions documented by the command, no local direct duplicate dependency); and `rtk cargo bench --bench throughput` (direct 1263.62 MiB/s, one-file pipeline 1237.75 MiB/s, many-file jobs=1 1194.74 MiB/s, default jobs 3861.26 MiB/s / 3.23x). The Windows/Linux CI runs are configured but have not been observed from this local implementation session; this is review evidence, not a claim of completed remote runs.
- 2026-08-20: Review-fix validation passed: `rtk cargo fmt --all -- --check`; `rtk proxy cargo clippy --all-targets --all-features -- -D warnings`; `rtk proxy cargo test --test watch_recursive -- --test-threads=1` (2 passed); `rtk proxy cargo test --test watch_stability -- --test-threads=1` (2 passed, including multi-period quiescence); `rtk cargo test --test metadata_provider_mock` through the approved local-loopback path (3 passed: exact/cover/cache, ambiguous search, and no-network); `rtk proxy cargo test --all-targets --all-features` (43 unit tests plus all integration suites and throughput executable passed); and `rtk proxy cargo build --release --locked` passed.
- 2026-08-20: Security re-review fix validation passed: `rtk cargo check`; elevated `rtk cargo test metadata::` (9 passed) and `rtk cargo test --test metadata_provider_mock` (1 passed, rejects local/IP/localhost/.local/untrusted/userinfo URLs without dispatch); `rtk proxy cargo clippy --all-targets --all-features -- -D warnings`; elevated `rtk proxy cargo test --all-targets --all-features` (47 unit tests plus all integration suites and throughput executable passed); and `rtk proxy cargo build --release --locked` passed.
- 2026-08-20: Independent re-review result was `Pass with Notes`; every Review Note below was independently re-inspected, re-tested, and closed. The reviewer independently observed 71 passing tests across 12 suites plus clean formatting, warnings-denied Clippy, and a locked release build.
- 2026-08-20: Root completion audit passed `rtk cargo fmt --all -- --check`, `rtk cargo clippy --all-targets --all-features -- -D warnings`, elevated `rtk cargo test --all-targets --all-features` (71 passed across 12 suites), `rtk cargo build --release --locked`, `rtk cargo tree --duplicates`, CLI help, and the complete supported-extension report. The final local benchmark reported direct QMCv1 1285.63 MiB/s, one-file pipeline 1230.62 MiB/s, many-file jobs=1 1031.88 MiB/s, and default jobs 3223.78 MiB/s (3.12x). A fresh release-mode NCM smoke produced a 16-bit stereo 44.1 kHz FLAC with SHA-256 `a0fd78ef14808cb8d78e6706a385adc834623e6692428f6ac4041b14c0077429`.

External validation note:

- Windows/Linux/macOS CI run links or captured results from the configured workflow.

## Review Notes

- [x] Major (2026-08-20, independent review): QMC footer resource/media identity is extracted into `PreparedDecrypt`, passed to staged metadata, and invokes exact lookup; identity-absent inputs alone use strict unique title/artist/duration search. `tests/metadata_provider_mock.rs` covers exact text/cover/cache, ambiguous search, and no-network behavior. Re-reviewed with 2026-08-20 source inspection and `rtk cargo test --all-targets --all-features` (69 passed).
- [x] Major (2026-08-20, independent review): `src/app/watch.rs` maintains normalized path fingerprints, queues only new/changed sources, removes deleted fingerprints, and coalesces running changes. `unchanged_reconciliation_is_quiescent_across_multiple_periods` proves a static tree is not reprocessed. Re-reviewed with targeted serialized watch tests.
- [x] Major (2026-08-20, independent review): `src/keys/qmc.rs` now ports the `go-mmkv` v0.1.4 payload/container and encrypted `.crc`/AES-128-CFB behavior, accepts `--qmc-mmkv-key`, and implements macOS v8/v10 discovery/derivation. Plain/encrypted vectors plus discovery helpers are tested; README and notices document the contract and attribution. Re-reviewed against the Go reference and `rtk cargo test keys::`.
- [x] Major (2026-08-20, independent review): text provenance remains unchanged while `merge` applies field-specific artwork order (existing embedded > container > exact > verified search). `existing_art_is_never_replaced_by_container_or_network_art` is the regression.
- [x] Minor (2026-08-20, independent review): `publish_temp_with` provides a test seam while production uses `fs::rename`; final-rename injection restores the old destination and removes the owned stage/backup artifacts. Re-reviewed with `rtk cargo test io::atomic::tests::`.
- [x] Major (2026-08-20, re-review): `MetadataClient` parses and validates every URL before cache/in-flight state or `Client::get`: production RPC permits only `https://u.y.qq.com`, and covers permit HTTPS/no-userinfo only from `y.gtimg.cn`, `imgcache.qq.com`, or `*.music.126.net`. IP literals, localhost, `.local`, HTTP, and untrusted origins fail best-effort before dispatch. The loopback transport is crate-private `cfg(test)` only; production integration coverage proves malicious NCM/QMC-origin cover URLs make no request, while staged audio remains unchanged and conversion callers retain their warning-only behavior. Re-reviewed by source inspection plus elevated `rtk cargo test --all-targets --all-features` (71 passed).

## Outcomes & Retrospective

The first Rust `unlockm` implementation now covers the planned cross-platform CLI surface, mirrored/flat batch processing, bounded parallel streaming, recursive resilient watch behavior, Go-parity format adapters and key sources, conservative native metadata enrichment, atomic publication, deterministic fixtures, and measured release performance. Independent review found five correctness/plan-drift issues and one SSRF issue; all were fixed and closed by the same reviewer before this plan moved to `Done`.

The remaining note is operational rather than an open implementation finding: the repository contains a Windows/macOS/Linux CI matrix, but this local session did not create a commit or observe hosted workflow runs because no commit/push was requested. Local macOS implementation, integration, security, watch, fixture, release, and performance evidence is recorded above.
