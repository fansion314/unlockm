# CLI Progress, Internal Crypto Migration, and Metadata Performance

## Status

Done

Created: 2026-08-21

Last Updated: 2026-08-21

## Objective

Make `unlockm` feel responsive during batch and watch processing, make releases independent of `lib_um_crypto_rust`, and remove the `--update-metadata` performance gap against the Go `um-cli` reference without weakening output correctness or safety.

The user-visible result is:

- An interactive batch run shows a stable overall progress bar and a second, single-refresh line naming the most recently active relative file. Warnings do not corrupt either line.
- The final result is concise and readable for a person, while redirected and JSON output remain stable for scripts.
- The release no longer fetches or builds `um_audio` or any `umc_*` crate from `https://git.um-react.app/um/lib_um_crypto_rust`.
- On the user's representative NCM library, `unlockm --update-metadata` reaches parity with the Go reference while retaining source files, same-run collision rejection, conservative metadata behavior, and atomic final publication.

The technical result is:

- A local `src/crypto/` boundary containing only the audio probing, cipher, header/footer, and KGM database primitives actually used by `unlockm`, with retained licensing and deterministic parity vectors.
- A cheap planning phase that resolves destinations and same-run conflicts without fully decrypting every input into long-lived staging files.
- A bounded execution phase that avoids redundant NCM parsing and removes the successful-path full-audio metadata candidate copy.
- Per-stage timings and benchmark coverage that distinguish crypto CPU, file IO, tag mutation, provider work, and publication rather than treating total wall time as one opaque number.

## Scope

### User-facing output

- Add TTY-aware progress rendering for the initial batch scan and conversion.
- Show overall completed/total file count, elapsed time, and current activity; use a second refresh line for the latest active relative path and active-worker count.
- After the initial batch, switch `--watch` to a spinner/status presentation with cumulative outcomes and the latest active path; an infinite watch stream does not claim a percentage or ETA.
- Add `--progress <auto|always|never>` with `auto` as the default. `auto` renders only on an interactive terminal and is disabled for JSON logs.
- Preserve a stable, one-line non-TTY summary. Emit a JSON summary object, not a trailing human line, under `--json-logs`.
- Improve Clap help text, value names, option relationships, and human-facing fatal/partial-success summaries.
- Keep progress and diagnostics on stderr so converted bytes or future stdout reports remain composable.

### Internal crypto migration

- Move the used portions of `um_audio`, `umc_ncm`, `umc_qmc`, `umc_kgm`, `umc_kuwo`, `umc_xiami`, and `umc_xmly` from `../lib_um_crypto_rust` revision `5dad838c33c3e4aab198d995160d3fbb9968470f` behind `src/crypto/`.
- Preserve the pinned algorithms and fixtures first; optimize only behind output-parity tests.
- Fold small shared utilities into `src/crypto/common.rs` or existing audited crates rather than recreating the upstream workspace layout.
- Remove the unused `umc_qtfm` dependency instead of migrating dead code.
- Update the format and key adapters to use `crate::crypto::*`; application, pathing, metadata policy, and publication code must not depend on cipher internals.
- Remove every `lib_um_crypto_rust` git source from `Cargo.toml` and `Cargo.lock`, and update notices/architecture accordingly.

### Pipeline and performance

- Extend deterministic benchmarks to cover NCM plus metadata, passthrough copy, fresh output, existing-output skip, and `--jobs` sensitivity.
- Add internal per-stage reports for classify/probe, decrypt, metadata resolve, tag write, fallback recovery, copy, and publish.
- Resolve output extensions with a bounded decrypted-prefix probe during planning. Do not materialize full decrypted stages until all same-run destination claims are known.
- Reuse parsed NCM/QMC/KGM state where practical and return a richer decrypt result containing detected extension, byte counts, and embedded metadata evidence.
- Mutate only a private, unpublished decrypted stage. On a tag-write failure that may have changed that stage, discard it and regenerate a clean stage from an unchanged source for best-effort publication. Network/merge failures that occur before mutation publish the original clean stage directly.
- Check non-overwrite destinations before expensive copy/decrypt work, while retaining a final publication-time recheck.
- Use an evidence-selected optimized filesystem copy path for passthrough files, followed by the same atomic publish and cancellation-before-publication checks.
- Tune disk-heavy concurrency only after a `--jobs 1/2/4/8` matrix; do not assume available CPU parallelism is the best APFS/HDD/network-volume write concurrency.
- Preserve bounded memory and source fingerprint/cancellation checks throughout fallback regeneration.

### Documentation and compatibility

- Update `ARCHITECTURE.md`, `README.md`, `THIRD_PARTY_NOTICES.md`, and CLI help for the new local crypto boundary, progress modes, reporting behavior, and measured concurrency defaults.
- Keep Windows, macOS, and Linux support and the current Rust 1.75 MSRV unless the chosen terminal dependency makes that impossible; any MSRV change is a recorded decision, not an incidental dependency update.

## Non-Goals

- Adding new encrypted formats, transcoding audio, or changing metadata eligibility/precedence rules.
- Removing general crates.io dependencies; the independence goal is specifically removal of the external `lib_um_crypto_rust` source dependency.
- Replacing Lofty wholesale before profiling proves that a narrower adapter improvement cannot meet the target.
- Sacrificing atomic publication, overwrite backup/restore, path containment, fail-closed registered formats, same-run collision rejection, or source retention for benchmark numbers.
- Localizing the CLI into multiple languages in this change. Human text remains clear English; localization can be a later presentation-layer extension.
- Showing every concurrently active filename. The requested single current-file line shows the latest activity plus the number of other active jobs.
- Publishing, pushing, or creating a release as part of implementation unless separately requested.

## Current Context

### Repository and reference state

- `unlockm` is at `c831a6d5b192bea5d8ec07d16701773c250a5706` on `main`, one commit ahead of `origin/main`, with no pre-existing uncommitted changes at plan creation.
- The completed initial plan is `docs/plans/2026-08-20-initial-rust-unlockm-cli.md`; the completed transactional tag plan is `docs/plans/2026-08-20-fix-extensionless-staged-metadata.md`.
- The Go reference is `../cli` at `d0d3647d615c8624abf27e30ade07d23da757041`.
- The Rust crypto reference is `../lib_um_crypto_rust` at the revision pinned in `Cargo.toml`, `5dad838c33c3e4aab198d995160d3fbb9968470f` (`v0.1.12`).
- The local crypto/audio source selected for migration is about 3,400 lines before trimming tests, unused public APIs, and shared helpers. `umc_qtfm` is declared but has no source/test/bench call site in `unlockm`.

### Current `unlockm` execution chain

1. `src/main.rs` parses `Cli`, initializes tracing, builds `RunConfig`, calls `app::run`, and prints only the final `converted/copied/skipped/conflicted/failed` line.
2. `src/app.rs::run` calls `scan::enumerate`. `src/app/scan.rs` walks the whole tree, creates mirrored empty directories, and returns a complete `Vec<PathBuf>`.
3. `src/app/runner.rs::run_paths_with_resources_and_cancel` starts a first blocking-worker phase named preflight, bounded by the `--jobs` semaphore (8 on the user's current machine).
4. `src/formats.rs::prepare` opens every file, reads up to 8 KiB, positively recognizes standard audio first, then validates registered encrypted suffix candidates. QMC v2 may reopen and reread its footer for metadata hints.
5. Passthrough files are only planned in this phase. Encrypted files immediately enter `src/app/job.rs::stage_decrypt`: a `.unlockm-stage-*` file is created under the output root, the source is opened again, parsed, and decrypted in 1 MiB chunks.
6. Every decrypt ends with `sniff_written`: flush, seek to byte zero, and reread up to 128 KiB to determine the native extension. `stage_decrypt` flushes again before metadata work.
7. With `--update-metadata`, `metadata::enrich_staged` content-probes the staged audio with Lofty. NCM then reopens and reparses the encrypted source to recover JSON and cover evidence even though the decrypt path already parsed the same NCM header.
8. If merged metadata is non-empty, `metadata::tags::prepare_write` creates `.unlockm-metadata-*`, performs a full `fs::copy` of the decrypted stage, content-probes the candidate, writes tags with Lofty, and content-probes/re-reads it again for round-trip verification. The original stage is retained solely as a safe fallback.
9. The runner waits for every preflight worker to finish and therefore retains all completed stages/candidates at once. It groups final destination claims and rejects every claimant in a same-run collision.
10. A second semaphore-bounded phase begins only after that global barrier. Passthrough copies use a user-space 1 MiB read/write loop into an atomic temp. Converted stages are renamed into place. Permissions/mtime are copied best effort, outcomes are accumulated, and nothing is rendered until the final summary.
11. `src/app/watch.rs` sends each stable event through the same runner with one path. The finite initial scan and the infinite watch loop currently share no presentation abstraction.

### Go reference execution chain

1. `cmd/um/main.go::processDir` recursively enumerates and processes files sequentially.
2. `processFile` chooses a decoder by suffix and positive validation. NCM `Validate` parses the content key, metadata, cover, and audio offset once and leaves one decoder positioned at audio.
3. `process` decrypts 64 bytes through the decoder for output sniffing and reconstructs the stream with `io.MultiReader`.
4. With metadata enabled, it writes decrypted audio once to a temporary file, obtains NCM metadata/cover from the already-prepared decoder, then writes the final FLAC through `go-flac` (or uses FFmpeg stream copy for other containers).
5. Ordinary files are copied directly with `io.Copy`. Existing outputs are checked before the final write. The Go path does not provide `unlockm`'s transactional metadata candidate, post-write round-trip verification, or atomic final replacement guarantees.

### Preliminary performance diagnosis

The user's fresh-output corpus result is the baseline:

- `unlockm --update-metadata`: 9.97 s wall, 1.34 s user, 4.98 s sys; `converted=80`, `copied=246`.
- Go `um --update-metadata`: 7.15 s wall, 3.69 s user, 2.75 s sys.
- Current input snapshot is about 3.4 GiB and includes 80 NCM files; counts can drift and must be captured for each comparison run.

Confirmed from the timings and code:

- Total recorded CPU is almost identical (6.32 s Rust versus 6.44 s Go), while Rust spends 2.23 s more in system time and 2.82 s more in wall time. The leading problem is IO/system scheduling, not evidence that the Rust cipher loop is computationally slower.
- All 80 encrypted files take the full stage-copy-tag-candidate path. The explicit full-audio metadata candidate copy, additional content probes, and repeated NCM parse are absent or arranged differently in Go.
- Eight disk-heavy jobs can decrypt, copy candidates, and shift/write tags concurrently. The old synthetic QMC benchmark was cache/CPU favorable and did not cover this metadata-heavy APFS workload.
- Every ordinary copy uses a manual loop plus a temporary rename, whereas Go can use its file-to-file `io.Copy` fast path and writes directly.
- The full-decrypt preflight barrier delays all passthrough work and retains a corpus-sized set of private stages until conflict grouping completes.

Hypotheses that require stage timing and benchmark evidence before choosing a final implementation:

- How much of the remaining gap is Lofty's FLAC mutation versus the explicit `fs::copy`.
- Whether APFS performs better with 1, 2, or 4 disk-heavy metadata writers than with the current default of 8.
- Whether a platform fast-copy path materially improves the 18 standard-audio/companion-file portion of the current corpus.
- Whether shared construction of the blocking HTTP client matters for QMC metadata workloads; it is not expected to dominate the reported NCM corpus.

## Design and Decisions

### D1. A presentation sink, not worker-owned terminal writes

Add `src/presentation.rs` with a small `ProgressSink`/event abstraction used by batch and watch orchestration. Workers report `Discovered`, `Started`, `PhaseChanged`, and `Finished` events containing relative paths and outcomes; only the sink owns terminal state. The no-op sink is used for non-TTY and test/library callers.

Interactive rendering uses two rows:

- Row 1: overall file-count progress, counts, elapsed time, and aggregate rate. File count is deliberately used instead of byte ETA because metadata and passthrough costs differ and byte callbacks in every decrypt chunk would add contention.
- Row 2: `Processing <relative path> (+N active)` with terminal-width truncation. The latest start/phase update wins this row.

Tracing must write through a progress-aware writer/suspension hook so warnings appear above the live rows. Renderer refresh is throttled; workers never redraw synchronously. `--progress always` uses an explicit draw target for manual/CI capture, while `auto` honors `IsTerminal`, `TERM=dumb`, and `NO_COLOR` behavior supplied by the selected terminal library.

### D2. Stable output modes

- Interactive human mode: live rows followed by a friendly elapsed summary such as `Done in 7.2s — 80 converted, 246 copied` plus failures/conflicts when nonzero.
- Redirected human mode: retain the current parseable `unlockm: converted=..., copied=..., skipped=..., conflicted=..., failed=...` summary and emit no control sequences.
- JSON mode: suppress live progress and emit the final counts, elapsed duration, and exit code as JSON. Existing JSON tracing events stay JSON; no plain trailing line is allowed.
- `--json-logs` with the default/explicit `--progress auto|never` is valid; explicitly combining `--json-logs` with `--progress always` is rejected as a conflicting output contract.
- Watch mode: finish the initial finite bar, then keep a spinner/status row until cancellation. It reports cumulative outcomes without a percentage.

### D3. Internal crypto boundary and migration method

Create `src/crypto/mod.rs` with format-oriented submodules (`audio`, `ncm`, `qmc`, `kgm`, `kuwo`, `xiami`, `xmly`) and a minimal `common` helper. Preserve upstream constants, transformations, errors, and vectors in the first migration commits. Each format adapter switches only after parity tests pass for that module.

The migration is selective:

- Keep APIs exercised by `src/formats.rs`, `src/formats/registry.rs`, `src/keys/kgm.rs`, fixtures, and benchmarks.
- Remove upstream CLI/WASM surfaces, unused formats, dead feature branches, and the unused QTFM dependency.
- Replace upstream `lazy_static` values with constants or `OnceLock` only when behavior is mechanically equivalent.
- Prefer slices and caller-owned buffers in hot transforms, but retain absolute-offset semantics and tiny-buffer tests.
- Keep the public format adapter boundary stable enough that crypto internals cannot leak into `app`, `metadata`, or `pathing`.

`THIRD_PARTY_NOTICES.md` records the exact source revision and the MIT/Apache-2.0 provenance of copied code and fixtures. Source headers identify substantial copied modules. The repository remains MIT while satisfying the retained notices.

### D4. Cheap planning before heavy execution

Split the current `preflight` concept:

1. Classification/planning reads only bounded source header/footer data and decrypts at most the prefix needed by audio detection. It returns a compact prepared descriptor and exact destination extension but no full staged audio.
2. The runner groups all destination claims and rejects every same-run claimant before expensive work.
3. Only conflict-free, non-skipped jobs enter the bounded execution pool. Their private stage is created beside the final destination, decrypted, optionally enriched, and published within the same job lifecycle.

The prefix probe may reopen and decrypt a small prefix again during execution if retaining prepared bytes for every path would make memory proportional to corpus size. Compact keys/headers may be retained; covers and audio prefixes may not accumulate unboundedly.

This preserves deterministic all-claimant collision rejection while removing the corpus-sized full-stage barrier and making progress correspond to actual completion.

### D5. Rich decrypt result and single container parse

Replace the string-only decrypt result with an internal report containing:

- detected extension and decrypted byte count;
- container identity/metadata/cover evidence already parsed while preparing or decrypting;
- source snapshot data needed to validate fallback regeneration;
- phase timings when diagnostic collection is enabled.

For NCM, parse container metadata and cover once per execution and pass typed evidence directly to `metadata::enrich_staged`; do not reopen the NCM source solely for metadata. QMC footer parsing/key selection similarly has one ownership path. Audio type detection uses the decrypted prefix already flowing through the transform rather than flushing, seeking, and rereading the output solely to sniff it.

### D6. Disposable private stage instead of a successful-path full copy

The final destination remains protected; the private stage itself does not need transactional identity.

- Metadata resolution, cover retrieval, validation, and merge happen before mutation. Any failure here leaves the clean decrypted stage publishable.
- Tag writing then mutates the unpublished stage in place and performs bounded round-trip verification.
- If mutation/save/verification fails after bytes may have changed, delete the private stage. If the source fingerprint is unchanged and the job is not cancelled, decrypt once more into a new clean stage and publish it without the failed metadata update. If the source changed, do not publish mixed/stale bytes; watch requeues it and batch reports a contextual failure or one bounded retry according to the source-snapshot policy.
- Tests inject post-save failure, fallback-decrypt failure, cancellation, and source-change cases. They assert the source and pre-existing destination are untouched, a successful stable-source metadata failure still publishes valid decrypted audio, and owned temps are cleaned.

This keeps the safety outcome introduced by the extensionless-stage fix while removing one full-audio copy from the common successful path. It deliberately spends a second decrypt only on the exceptional failed-mutation path.

### D7. Fast skip/copy with final safety checks

After exact destination planning, a non-overwrite job may return `Skipped` before decrypt/copy if the destination already exists. Publication still rechecks to close races.

For passthrough, benchmark `std::fs::copy`/platform-assisted file-to-file copying into an owned same-directory temp against the current manual loop. The chosen fast path must preserve:

- no publication after cancellation;
- no truncation of an existing destination;
- owned-temp cleanup on failure;
- permissions/mtime behavior and bounded process memory;
- a safe chunked fallback on unsupported filesystems/platforms.

Do not add a platform-specific syscall merely because it is theoretically faster; keep it only if the representative benchmark shows a material gain and cross-platform fallbacks pass.

### D8. Evidence-gated concurrency and completion bar

Run fresh-output, warm-cache, counterbalanced trials for jobs 1, 2, 4, and 8. Separate:

- cipher-only synthetic QMC;
- deterministic large NCM with metadata and cover;
- passthrough copy;
- the user's local mixed corpus;
- repeated run with existing destinations.

Record median of at least three measured trials, hardware/filesystem, corpus counts/bytes, warning/network behavior, wall/user/sys time, peak RSS, and logical stage bytes. Alternate Go/Rust order to reduce cache bias.

Completion performance gates on the user's machine are:

- Fresh-output `--update-metadata` median wall time is no more than 1.05x the same-run Go reference and at least 20% faster than the current 9.97 s Rust baseline.
- Rust system time falls by at least 25% from the current 4.98 s baseline unless platform counters demonstrate equivalent reduced IO with a timer artifact.
- No-metadata wall time and the direct cipher benchmark regress by no more than 5%.
- Peak process memory remains approximately `jobs * buffer_size + 128 MiB`, excluding OS page cache and bounded provider/key indexes.
- If atomic/safety guarantees make Go parity impossible after the measured copy removal and concurrency tuning, implementation stops at `In Review` with the remaining stage costs quantified; the plan is not marked `Done` by weakening those guarantees.

## Milestones

### M1. Measurement and reporting foundation

Goal: make the current gap reproducible and make later optimizations attributable.

Dependencies: none.

Acceptance criteria:

- Benchmarks cover cipher-only, NCM metadata, passthrough, and existing-output cases with output integrity assertions.
- Per-job diagnostic reports expose stage durations and bytes without changing default logs.
- The current and post-change Go/Rust comparison protocol is recorded and repeatable.

Validation:

- `rtk cargo bench --bench throughput`
- `rtk cargo bench --bench metadata_pipeline`
- `rtk cargo test --test logging --test batch_layout --test metadata_staging`
- A three-run local baseline matrix recorded under `Completion Evidence`.

### M2. Self-contained crypto/audio implementation

Goal: remove the git crypto dependency without changing supported bytes or format behavior.

Dependencies: M1 baseline.

Acceptance criteria:

- All used crypto/audio modules live under `src/crypto/` with notices and module-level attribution.
- Existing format, key, suffix, tiny-buffer, footer/header, and end-to-end vectors pass before and after each adapter switch.
- `Cargo.toml` and `Cargo.lock` contain no `lib_um_crypto_rust`, `um_audio`, or `umc_*` package source.
- Unused QTFM code is removed, not silently carried forward.

Validation:

- `rtk cargo test formats::`
- `rtk cargo test keys::`
- `rtk cargo test --test decrypt_fixtures --test metadata_staging --test no_network_by_default`
- `rtk rg -n 'git.um-react.app/um/lib_um_crypto_rust|um_audio =|umc_[a-z0-9_]+ =' Cargo.toml Cargo.lock` (expect no matches)
- `rtk cargo tree --locked --duplicates`
- `rtk cargo build --release --locked`

### M3. Metadata-heavy pipeline optimization

Goal: reduce full-file IO and system time while preserving all safety invariants.

Dependencies: M1-M2.

Acceptance criteria:

- Full decrypted stages are created only for conflict-free jobs in the execution pool and are not retained behind a batch-wide heavy-work barrier.
- NCM container metadata is not reparsed after decrypt, and output sniffing does not reread 128 KiB from the completed stage.
- Successful tag updates perform no full-stage candidate copy.
- Failed tag mutation regenerates clean audio only from an unchanged source; cancellation/source changes cannot publish stale or partial output.
- Existing-output skip happens before expensive work, and passthrough uses the measured fast path.
- Performance gates in D8 pass or the quantified remaining gap is held for review rather than hidden.

Validation:

- `rtk cargo test --test batch_layout --test failure_isolation --test metadata_roundtrip --test metadata_staging`
- `rtk cargo test io::atomic::tests::`
- `rtk cargo test metadata::tags::`
- `rtk cargo test formats::`
- `rtk cargo test --test watch_recursive --test watch_stability -- --test-threads=1`
- `rtk cargo bench --bench throughput`
- `rtk cargo bench --bench metadata_pipeline`
- Counterbalanced release comparison against the Go reference on the local NCM corpus.

### M4. Interactive progress and friendly output

Goal: provide responsive terminal feedback without corrupting logs or machine output.

Dependencies: M1 job-report events and M3 final job lifecycle.

Acceptance criteria:

- Interactive batch mode displays exactly the two requested live rows and completes every discovered path exactly once.
- Warnings/errors render cleanly above the progress rows; long/Unicode paths are truncated safely.
- Non-TTY output has no ANSI/carriage-return animation and retains the stable count summary.
- JSON mode produces JSON events plus a JSON summary only.
- Watch mode transitions from the finite initial bar to an indefinite status/spinner and shuts down cleanly.
- CLI help describes all currently terse flags and progress behavior.

Validation:

- `rtk cargo test --test progress_output --test logging --test cli_exit_codes`
- `rtk cargo test presentation::`
- `rtk cargo test cli::`
- `rtk cargo test app::runner::`
- Manual TTY smoke with success, warning, partial failure, `--progress never`, `--progress always`, JSON logs, a narrow terminal, Unicode paths, and Ctrl-C watch shutdown.

### M5. Final compatibility, documentation, and performance audit

Goal: prove the combined change is releasable and durably documented.

Dependencies: M1-M4.

Acceptance criteria:

- All local tests, strict Clippy, formatting, locked release build, benchmarks, license/dependency checks, and representative smokes pass.
- `ARCHITECTURE.md`, `README.md`, notices, help text, and this plan describe the implemented boundaries and measured defaults.
- Windows/macOS/Linux CI is green or any unobserved remote run remains an explicit validation gap.
- No review finding contradicts output safety, metadata semantics, dependency independence, or the performance target.

Validation:

- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy --all-targets --all-features -- -D warnings`
- `rtk cargo test --all-targets --all-features`
- `rtk cargo build --release --locked`
- `rtk cargo tree --locked --duplicates`
- `rtk cargo bench`
- Fresh NCM release smoke with and without metadata, plus final Go comparison.

## Task Breakdown

### M1 tasks

- [x] T1.1 Extract deterministic NCM fixture generation into a reusable test/benchmark helper and add `benches/metadata_pipeline.rs` covering metadata success, invalid-cover best effort, and output payload identity; verify the new bench in release mode.
- [x] T1.2 Extend `JobOutcome`/runner reporting with source-relative path, phase timings, logical bytes, and final outcome while keeping `Summary` compatibility; verify unit and failure-isolation tests.
- [x] T1.3 Add debug/trace-only aggregate timing output and document the counterbalanced Go/Rust benchmark script/commands in this plan; record current jobs 1/2/4/8 baselines before optimization.

### M2 tasks

- [x] T2.1 Add `src/crypto/{mod,common,audio}.rs`, port only used audio detection and shared helpers with upstream vectors/attribution, switch `src/formats/audio.rs`, and verify audio/registered-passthrough tests.
- [x] T2.2 Port NCM into `src/crypto/ncm/`, retain content-key/header/metadata vectors, switch NCM format/metadata call sites **and the shared test/benchmark NCM fixture builder to `crate::crypto::ncm` (or a static pre-encrypted vector)**, and verify NCM decrypt plus tag-staging tests.
- [x] T2.3 Port QMC v1/v2/footer/ekey modules into `src/crypto/qmc/`, retain tiny-buffer/footer/key vectors, switch format/key/benchmark fixtures, and verify every QMC alias and key source.
- [x] T2.4 Port KGM plus database decryption into `src/crypto/kgm/`, switch `src/keys/kgm.rs`, and verify v2/v3/v5/header/SQLite-key behavior.
- [x] T2.5 Port the used Kuwo, Xiami, and Ximalaya modules, switch registry/stream adapters, and verify positive, corrupt, shared-`.xm`, and tiny-buffer cases.
- [x] T2.6 Remove all external crypto/audio git dependencies and unused QTFM declarations from manifests/lockfile, reconcile dependency versions, update `THIRD_PARTY_NOTICES.md`, and verify a locked release build plus a dependency-source search that covers `Cargo.toml`, `Cargo.lock`, `src/`, `tests/`, and `benches/`.

### M3 tasks

- [x] T3.1 Refactor `formats::PreparedDecrypt` and `app::job` into bounded destination planning plus heavy execution; resolve extensions from bounded prefix probes, reject all collision claimants before full decrypt, and add collision/memory regressions.
- [x] T3.2 Carry the validated planned extension and per-execution NCM evidence through the adapter/job boundary, split `JobTimings` by phase, and remove duplicate post-decrypt NCM parsing and completed-stage sniff rereads; verify byte-identical format fixtures.
- [x] T3.3 Split metadata resolve from mutation and add private-stage in-place tag writing with source-snapshot fallback regeneration; inject pre-mutation, post-save, fallback-decrypt, cancellation, and source-change failures and verify atomic/source invariants.
- [x] T3.4 Add early non-overwrite skip and a benchmark-selected optimized passthrough-to-temp copy path with final cancellation/publication checks; verify existing-output, overwrite, permissions/mtime, cleanup, and watch cancellation.
- [x] T3.5 Remove duplicate cover decoding and, only if measured outside the NCM workload, reuse provider client construction safely within blocking resources; verify security allowlists, cache/in-flight behavior, and no-network defaults.
- [x] T3.6 Run the full benchmark matrix, tune disk-heavy concurrency/defaults from evidence, record median wall/user/sys/RSS/stage-byte results, and keep any unmet D8 gate visible.

### M4 tasks

- [x] T4.1 Add progress mode parsing/config and no-op/terminal presentation sinks in `src/presentation.rs`, using forced-mode pipe capture for deterministic tests; verify auto/always/never, TTY, JSON, dumb-terminal, color-free, and narrow-width decisions.
- [x] T4.2 Feed scan, planner, batch execution, collision/failure, and watch lifecycle events into the presentation sink without worker-owned drawing; verify exact discovered/completed accounting under parallelism and cancellation.
- [x] T4.3 Integrate tracing with progress suspension/throttled redraw so warnings and Unicode/long current paths do not corrupt live rows; verify with deterministic terminal-output assertions.
- [x] T4.4 Add friendly interactive and JSON summaries while preserving the redirected human summary, expand all Clap help text, and add `tests/progress_output.rs` plus logging/exit-code regressions.
- [x] T4.5 Repair live completion delivery so each worker advances the batch bar before the scheduler admits the next queued file; add a multi-file `--jobs 1` regression that distinguishes live advancement from an end-of-run result-drain burst.

### M5 tasks

- [x] T5.1 Reconcile `ARCHITECTURE.md`, `README.md`, `THIRD_PARTY_NOTICES.md`, and plan decisions with the implemented local crypto, runner, metadata recovery, progress, and benchmark boundaries.
- [x] T5.2 Run format/test/lint/build/dependency/license validation and a local NCM smoke with/without metadata; record exact commands, counts, hashes/type checks, and all skipped coverage.
- [x] T5.3 Run and record the final counterbalanced Go comparison and performance gates on the user's corpus; confirm no-metadata performance did not regress.
- [x] T5.4 Perform independent review, resolve or explicitly accept every finding, update `Progress`, `Discoveries`, `Decision Log`, and `Completion Evidence`, and move to `Done` only when `docs/PLANS.md` permits.

## Validation Strategy

### Correctness and safety

- Retain deterministic upstream crypto vectors and compare detected type, payload bounds, decrypted SHA-256, metadata evidence, and tiny-buffer output across the migration.
- For tag mutation, compare container-aware audio payload fingerprints rather than whole-file hashes because valid tag layout changes container bytes.
- Inject failures before tag mutation, after save, during verification, during fallback decrypt, and during final rename/restore. Assert source retention, old-destination retention, valid fallback output when promised, and zero leaked `.unlockm-*` artifacts.
- Re-run flat/mirror collision, child-output pruning, overwrite, in-place passthrough, failure isolation, and watch cancellation/requeue suites after the runner split.
- Keep network metadata deterministic through the existing loopback seam; a live provider call is optional evidence, not a test gate.

### Presentation

- Unit-test renderer state with an injected terminal width/capability and memory draw target; do not make CI correctness depend on a real PTY.
- Integration-test captured non-TTY stdout/stderr for absence of ANSI/control refreshes and preservation of stable summaries.
- Test JSON by parsing every emitted line expected to be JSON, including the final summary.
- Manually inspect a real TTY for two-row stability, progress-aware warnings, long Unicode names, terminal resize, and graceful watch cancellation.

### Performance protocol

- Build both release binaries from recorded commits. Use fresh output directories on the same APFS volume and record exact binary hashes/versions.
- Warm source page cache once, then alternate Go/Rust order for at least three measured trials per relevant job count. Never compare a fresh-cache run to a warm-cache run.
- Record corpus file count, extension count, logical bytes, output filesystem, active flags, network/warning count, wall/user/sys time, and peak RSS.
- Keep the synthetic benchmark large enough that process startup is insignificant and validate output after timing.
- Report both safety-adjusted production performance and raw cipher throughput. Do not infer a cipher regression from storage saturation or claim a storage improvement from page-cache-only data.

### Required final commands

- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy --all-targets --all-features -- -D warnings`
- `rtk cargo test --all-targets --all-features`
- `rtk cargo build --release --locked`
- `rtk cargo tree --locked --duplicates`
- `rtk cargo bench`
- `rtk proxy ./target/release/unlockm --help`
- `rtk proxy ./target/release/unlockm --supported-ext`
- Dependency-source `rtk rg` checks from M2.
- Counterbalanced `/usr/bin/time -l` release runs from D8, using explicit `mktemp -d` output roots.

## Risks and Mitigations

- **Copied crypto code can silently diverge from the reference.** Freeze the exact revision in notices, port one format per commit, run old/new parity vectors before removing each dependency, and avoid speculative algorithm rewrites.
- **Removing the upstream crate can accidentally broaden local public APIs.** Keep all crypto modules crate-private and expose only prepared format operations through `formats`.
- **Progress redraw can corrupt tracing or JSON.** Centralize terminal ownership, use a progress-aware tracing writer, disable animation in JSON/non-TTY auto mode, and parse every JSON test line.
- **Concurrent files make “current file” ambiguous.** Define it as the most recent worker phase/start update and show the other-active count; never imply only one file is running.
- **In-place stage tag mutation can leave the private stage corrupt.** Treat the stage as disposable, regenerate only from an unchanged source, and never publish after mutation failure without a clean fallback verification.
- **Fallback regeneration can race a changing watch source.** Capture and recheck size/mtime (and existing watch generation), cancel/requeue changed paths, and never combine bytes from two source generations.
- **Prefix planning can make memory proportional to file count.** Retain only compact parsed state; discard large decrypted prefixes/covers and re-read a bounded prefix during execution where necessary.
- **Lower IO concurrency can hurt fast NVMe/no-metadata throughput.** Keep `--jobs` explicit, benchmark metadata and non-metadata separately, and tune defaults or add an internal disk-heavy limit only when both results justify it.
- **Fast filesystem copy may delay cancellation.** Check before and after the kernel copy, never publish after cancellation, retain a chunked fallback where responsiveness is required, and measure large-file watch behavior.
- **Go is faster partly because it offers weaker publication guarantees.** Compare honestly, remove redundant work first, and hold an unexplained residual gap for review rather than deleting safety checks.
- **Real-corpus timing is noisy or network-dependent.** Pair it with deterministic local fixtures, alternate run order, capture network/warnings, and use medians rather than a single number.
- **Terminal dependencies can raise MSRV or binary size.** Check MSRV and release size before adoption; prefer a small renderer abstraction and pin compatible versions.
- **Licensing can become unclear after selective copying.** Retain MIT/Apache-2.0 attribution per substantial module/fixture and audit notices before final review.

## Progress

- [x] 2026-08-21: Read `AGENTS.md`, `docs/PLANS.md`, `ARCHITECTURE.md`, both completed ExecPlans, CI guidance, README, manifests, runner/job/format/metadata/atomic/watch code, and relevant tests/benchmarks.
- [x] 2026-08-21: Compared the current Rust path with Go `cmd/um/main.go`, NCM decoder, passthrough copy, and FLAC/FFmpeg metadata paths.
- [x] 2026-08-21: Confirmed the user timing points to system/IO overhead: nearly equal total CPU, but +2.23 s Rust system time and +2.82 s wall time.
- [x] 2026-08-21: Inventoried the external crypto surface and confirmed `umc_qtfm` has no current call site.
- [x] 2026-08-21: Created this Ready plan. No production source was changed.
- [x] 2026-08-21: M1 complete — deterministic NCM/metadata benchmark and internal terminal reports are implemented; focused tests and both release benchmarks pass; a three-run fresh-output jobs=1/2/4/8 matrix is recorded below. Ready for independent M1 review before M2.
- [x] 2026-08-21: M1 review notes resolved by independent reviewer — verified (1) default `RunnerMetrics` keeps O(1) totals and only tests collect bounded reports, (2) fixture's temporary `umc_ncm` dependency is accurately scoped to T2.2/T2.6, and (3) Failed/Conflict terminal-report regressions pass. Cancellation and blocking-worker-panic terminal accounting deliberately remain an M4 presentation-lifecycle gap.
- [x] 2026-08-21: M2/T2.1 complete — ported `um_audio` detection and byte-order helpers into crate-private `src/crypto/{audio,common}.rs`; added parity coverage for multi/large ID3, invalid MP3/junk, MP4-family magic, and MP3/FLAC/Ogg/M4A adapter fixtures.
- [x] 2026-08-21: M2/T2.2 complete — ported NCM header/content-key/metadata decrypt into `src/crypto/ncm/`, switched format and metadata adapters, and replaced the external-test builder with a retained static upstream NCM cipher-stream vector. `src/`, `tests/`, and `benches/` contain attribution text only, no `umc_ncm` code call.
- [x] 2026-08-21: M2/T2.3 complete — ported pinned QMC v1, v2 map/RC4, EKey/TEA, and all Android/PC footer parsers into `src/crypto/qmc/`; switched production format/footer/key paths and replaced integration/benchmark encryption with an independent static-key fixture helper. The direct `umc_qmc` dependency is removed; remaining lockfile occurrences are transitive through not-yet-migrated KGM/Kuwo.
- [x] 2026-08-21: M2/T2.4 complete — ported pinned KGM header, slot keys, v2/v3/v5 decipher, and encrypted SQLite page-key/decrypt logic into `src/crypto/kgm/`; KGM v5 uses the local QMCv2 EKey cipher and `src/keys/kgm.rs` uses the local database decryptor. The direct `umc_kgm` dependency is removed.
- [x] 2026-08-21: M2/T2.5 complete — ported the used Kuwo KWM v1/v2 header/cipher path, Xiami header/cipher, and Ximalaya Android/PC header decrypt into crate-private modules; pruned unused Kuwo service DES/BoDian APIs. Added pinned X2M/X3M encrypted/plain header vectors plus positive Kuwo/Xiami tests and retained corrupt/shared-`.xm` adapter coverage.
- [x] 2026-08-21: M2/T2.6 complete — removed every `lib_um_crypto_rust` package and the unused QTFM declaration from `Cargo.toml`/`Cargo.lock`, reconciled direct crates.io crypto dependencies, and updated architecture/notices. Locked release build and source/dependency audits are clean; M2 is ready for independent review before M3.
- [x] 2026-08-21: M2 review notes resolved by independent closing review — reviewer verified the KGM v2/v3/header/KDF/encrypted page/page-1, QMC PCv2/EncV2, KWMv2, and Ximalaya PC replacement vectors plus the architecture correction. M2 passed with only the already-deferred M4 cancellation/panic lifecycle note.
- [x] 2026-08-21: M3/T3.1–T3.6 implemented — bounded prefix planning now precedes collision grouping; only conflict-free jobs create destination-adjacent stages. Planned extensions avoid completed-stage rereads, NCM evidence is job-local, successful FLAC metadata writes are streaming/in-place, failure recovery is source-snapshot/cancellation gated, existing outputs skip before heavy work, and passthrough uses `io::copy`. Final performance/memory gates are recorded below. Ready for independent M3 review before M4.
- [x] 2026-08-21: M3 review blocker resolved by independent closing review — reviewer verified final decrypt/copy cancellation and source-fingerprint gates, dirty-running watch cancellation with fresh requeue token, destination preservation, temp cleanup, and FLAC picture preservation. M3 passed with a non-blocking note that a future full-execute race seam would strengthen the existing helper-boundary regression.
- [x] 2026-08-21: M4/T4.1–T4.4 implemented — TTY `auto|always|never`, two-row batch status, percentage-free watch status, progress-aware tracing, Unicode/narrow-width current paths, human/non-TTY/JSON summaries, expanded help, and cancellation/panic terminal accounting are wired through the shared runner. Automated output tests and a real PTY watch/Ctrl-C smoke pass. Ready for independent M4 review before final M5 audit.
- [x] 2026-08-21: M4 review note resolved by independent closing review — reviewer verified `NO_COLOR` disables auto progress, `always` remains explicit override, JSON remains control-free, and pre-start errors emit no cursor controls. M4 passed with no remaining findings.
- [x] 2026-08-21: M5/T5.1–T5.3 complete — architecture/README/notices/help/plan match the implementation; final fmt, strict Clippy, elevated full tests, locked release build, dependency/source audit, local with/without-metadata NCM smoke, stream identity, final Rust/Go matrix, no-metadata shield, and RSS gate are recorded. Status moved to In Review for T5.4 independent final audit.
- [x] 2026-08-21: T5.4 final independent review passed with only the accurately recorded sandbox/sccache environment note. Root completion audit then found and fixed a TM fail-closed prefix-probe gap; reviewer re-opened the delta and explicitly confirmed the final Pass. All tasks/findings are closed and Status is Done.
- [x] 2026-08-21: Reopened after a real 326-file TTY run showed `0/326` throughout decryption and an impossible `+81 active` display. Completed workers now emit `finished()` immediately; the new serial multi-file regression proves `1/3` is rendered before a later file starts. Focused progress/runner tests, the 124-test full matrix, strict Clippy, locked release build, formatting, and diff checks all pass; Status returned to Done.

## Discoveries

- The prior QMC throughput benchmark supported the 8-job default on a 64 MiB synthetic cache-friendly workload, but it did not exercise NCM parsing, Lofty metadata writes, full-stage candidate copies, or a 3.4 GiB mixed directory. Its conclusion cannot be reused as evidence for metadata-heavy IO concurrency.
- `--update-metadata` currently reparses an NCM container after decryption and copies the entire decrypted audio before tag mutation. These are concrete redundant operations independent of any cipher implementation choice.
- The all-heavy-work preflight exists to reject every same-run collision claimant after exact native extension detection. A bounded decrypted-prefix planning probe can preserve that semantic without retaining every full stage.
- The transactional metadata fix protects a private stage as though it were the final artifact. Equivalent user safety can be retained more cheaply by treating that stage as disposable and regenerating from the immutable source only on exceptional post-mutation failure.
- The current default help output leaves nearly every option without an explanation, so the user-friendly output work includes help and summary design, not only an animated bar.
- The current `--json-logs` mode still receives a plain final `eprintln!` summary. A presentation boundary is needed to make output mode internally consistent.
- Go's reported warnings do not explain its advantage; it is faster despite producing them. The timing split points instead to Rust's IO path.
- The current source snapshot contains about 3.4 GiB and 80 NCM inputs, but corpus companion counts may drift. Every benchmark record must capture the actual snapshot rather than rely on the earlier `copied=246` count.
- `RunConfig::new` canonicalizes `/var` temporary paths to `/private/var` on macOS. Bench runners must submit `config.input.clone()` rather than the pre-normalization path or the runner correctly rejects it as outside the input root.
- The new large deterministic benchmark retains the silent fixture's single ID3/header prefix and repeats its valid MPEG payload. Default 32 MiB NCM payloads therefore exercise real tag probing/mutation and IO; `UNLOCKM_NCM_BENCH_BYTES` may reduce it (minimum 64 KiB) only for quick validation.
- Current M1 reports cover terminal normal-path/preflight-failure outcomes. Cancellation deliberately retains the existing zero-count `Summary` contract, and a blocking-worker panic has no source-level terminal report; M4 must introduce explicit cancellation lifecycle events before claiming exact discovered/completed presentation accounting.
- M1 review found that the first `RunnerMetrics` implementation retained an unbounded `Vec<JobReport>` by default, violating the bounded-memory invariant. The repaired default must retain only O(1) aggregate totals; per-path collection is explicit and bounded/test-only.
- The initial M1 fixture builder still called pinned `umc_ncm`; T2.2 closed that review boundary by replacing the call with the fixed upstream header's independent 256-byte stream vector. Tests/benchmarks now generate NCM inputs without calling production or external crypto.
- During T2.3, an abbreviated footer draft omitted the pinned parser's typed errors and PC/MusicEx branches. It was rejected before task completion and replaced from the exact pinned production sources; future format ports must preserve the full source semantics before API trimming.
- The pinned KGM v2/v3 header fixtures are constructor/header vectors: upstream tests do not run the higher-level challenge self-test on them. Local replacements therefore assert exact header fields, constructor success, and whole-versus-1/7/16/4096-byte transform parity; the retained synthetic v5 test continues to exercise the challenge self-test.
- Lofty 0.21's FLAC writer calls `read_to_end` on the full staged file and caused metadata peak RSS of 460.8 MB at jobs=2 (417.8 MB even at jobs=1), while the same corpus without metadata used 17.9 MB. A bounded FLAC block writer now preserves unknown comments/non-front pictures, uses padding when possible, shifts audio frames in 1 MiB chunks when necessary, and reduced metadata RSS to 39.1 MB.
- `/usr/bin/time -lp` inside the filesystem sandbox reports correct real/user/sys values and successful unlockm counts but exits 1 after `sysctl kern.clockrate` is denied. One escalated `time -l` run supplied authoritative RSS/footprint and exited 0; benchmark tables use the successful binary summaries and recorded timer values, not the wrapper exit code.
- A small application-owned renderer avoids adding a terminal dependency/MSRV change. Tracing writers buffer one formatted event, clear the live rows once, write the event, and redraw; non-TTY/JSON modes skip all control sequences. `--progress always` provides deterministic pipe capture for integration tests.
- The progress renderer itself computes counts correctly. The live stall came from event delivery: the publish admission loop waited on semaphore permits while completed results accumulated undrained, so starts were visible immediately but finishes were deferred into a final burst.

## Decision Log

- **2026-08-21 — One integrated plan.** Progress reporting depends on runner lifecycle events, and the best performance refactor depends on owning internal prepared cipher state. The three requested improvements remain one coordinated plan with separately reviewable milestones.
- **2026-08-21 — Ready without additional product questions.** Progress semantics, JSON/non-TTY behavior, watch behavior, internal-module scope, performance gates, and safety constraints are explicit; implementation can begin.
- **2026-08-21 — Preserve stronger safety than Go.** Go is the throughput comparator, not the output-safety specification. Atomic publication and recovery stay release gates.
- **2026-08-21 — Selective internalization.** Migrate only used audio/cipher primitives and their tests; remove unused QTFM rather than recreating the whole sibling workspace.
- **2026-08-21 — Parity before optimization.** Each crypto family first lands byte-compatible, then prepared-state/allocation improvements land under the same vectors.
- **2026-08-21 — File-count progress.** The default progress bar advances per completed input; this avoids hot-loop byte-event contention and misleading mixed-workload byte ETAs.
- **2026-08-21 — Latest activity for the single filename row.** Parallel jobs remain enabled; the row shows the latest job/phase and active count rather than serializing work for presentation.
- **2026-08-21 — Disposable stage recovery.** Successful metadata mutation occurs on the private stage without a full candidate copy; a post-mutation failure regenerates clean audio only from an unchanged source.
- **2026-08-21 — Measured default concurrency.** Available CPU parallelism is no longer assumed to equal optimal metadata IO parallelism. Final defaults follow the recorded matrix.
- **2026-08-21 — Performance gate is relative and safety-aware.** Aim for no worse than 1.05x Go median on the representative metadata workload while holding no-metadata regression to 5%; do not mark `Done` by weakening invariants.
- **2026-08-21 — M1 starts with deterministic local evidence.** The first benchmark uses the existing legal silent-MP3 and upstream NCM-header vectors, so success/failure metadata paths have no network or personal-library dependency.
- **2026-08-21 — Current report phases are intentionally coarse.** M1 exposes `preflight` and `publication` without logging by default; M3 will split the former into classify/probe, decrypt, metadata resolve, tag write, and fallback recovery once the pipeline itself is refactored.
- **2026-08-21 — M1 jobs=2 evidence was provisional.** One fresh M1 matrix favored two disk-heavy workers but did not change defaults; the final counterbalanced M3 matrix later confirmed and promoted `min(2, available_parallelism)`.
- **2026-08-21 — Review repair: opt-in report retention.** Normal runs aggregate timing/bytes/counts in O(1) state and trace each terminal report directly. Tests and a future explicit diagnostics mode may opt into bounded collected reports; no normal batch retains every source path.
- **2026-08-21 — Fixture migration is part of T2.2.** T2.2 must switch the NCM fixture builder/benchmark to `crate::crypto::ncm` or static pre-encrypted vectors before the external dependency is removed. T2.6's source audit includes `tests/` and `benches/`, not only manifests.
- **2026-08-21 — Review repair: bounded metrics by default.** `RunnerMetrics::default()` owns only scalar lifecycle and aggregate timing state. It emits each transient `JobReport` immediately at trace and emits a debug aggregate from O(1) totals; the bounded `Vec<JobReport>` collector is compiled only for runner unit tests.
- **2026-08-21 — M1 closing review passed with notes.** The reviewer independently rechecked the three M1 findings and their focused validation. M2 may proceed; cancellation/panic terminal events are intentionally deferred to the M4 presentation lifecycle rather than expanding M1 reporting semantics.
- **2026-08-21 — NCM fixture uses a static stream vector.** Integration-test crates cannot call crate-private crypto modules. To avoid widening the production crypto API, T2.2 replaces `umc_ncm` in the shared fixture with the fixed upstream header's retained 256-byte stream vector; format tests exercise the actual local NCM parser/decrypt path.
- **2026-08-21 — QMC remains crate-private.** Production QMC primitives are not exposed for integration fixtures. Tests and throughput generation use an independently retained pinned QMCv1 static-key transform, while format unit vectors exercise the local v2 EKey/footer/cipher path.
- **2026-08-21 — Port only used Kuwo surfaces.** KWM needs header parsing plus v1/QMCv2 payload ciphers; service-only DES EKey and BoDian APIs are not application callsites and were removed after strict Clippy identified the dead surface. Ximalaya and Xiami similarly retain only adapter-reachable operations and parity vectors.
- **2026-08-21 — Independent review vectors may use inverse primitives, not production helpers.** KGM encrypted DB pages, EncV2 wrappers, KWMv2 payloads, and Ximalaya PC two-stage headers are built in tests from pinned/static inputs and crates.io AES/TEA inverse operations; assertions execute the local production decrypt/parser path. Static upstream v2/v3/MusicEx/X2M/X3M assets remain base64 text with notice attribution.
- **2026-08-21 — M2 closing review passed with notes.** The independent reviewer closed the missing-vector and stale-architecture findings after inspecting the repair and independently running crypto/format/full tests and dependency audits. The remaining note belongs to M4 presentation cancellation/panic lifecycle, not crypto migration.
- **2026-08-21 — M3 carries evidence outside a monolithic decrypt result.** Retaining NCM covers or decrypted prefixes in every planned job would make memory corpus-proportional. The bounded probe returns only the exact extension; compact QMC hints remain planned state, while NCM JSON/cover evidence is parsed inside the admitted execution job and reused after decrypt. Per-phase `JobTimings` carries classify/probe, decrypt, resolve, tag, fallback, copy, and publish measurements.
- **2026-08-21 — Real mixed IO defaults to two jobs.** The final APFS matrix shows jobs=2 at 2.99 s median versus jobs=1 3.62 s, jobs=4 3.39 s, and jobs=8 5.66 s. `--jobs` now defaults to `min(2, available_parallelism)`; explicit higher values remain available for cache/CPU-heavy workloads, whose 64 MiB QMC benchmark still scales at jobs=8.
- **2026-08-21 — Streaming FLAC mutation is the bounded common path.** NCM corpus outputs use a crate-local FLAC metadata-block writer; other containers continue through Lofty and retain the private-stage regeneration safety net. Tests force both metadata growth/audio shifting and padding reuse while comparing unchanged audio-frame bytes.
- **2026-08-21 — Source identity is a publication precondition.** Decrypt and passthrough jobs capture source size/mtime before heavy work and recheck it with cancellation immediately before atomic publish. A running watch fingerprint change cancels the current token and schedules one clean follow-up; stale stages are dropped and never made visible.
- **2026-08-21 — M3 closing review passed with notes.** The independent reviewer closed the stale-publication Blocker by code inspection and verified the new test boundaries. A future end-to-end race hook may supplement the deterministic helper-level test, but it is not required for M3 correctness.
- **2026-08-21 — Presentation owns terminal state.** Workers emit planning/start/finish lifecycle updates through the cloneable reporter and never write ANSI directly. The tracing subscriber uses the same reporter as its `MakeWriter`; JSON disables progress, and forced progress plus JSON is a configuration error.
- **2026-08-21 — M4 closing review passed.** The independent reviewer closed the NO_COLOR finding and found no remaining blocker, major, or meaningful minor issue in progress/output behavior.
- **2026-08-21 — Final review and completion audit passed.** The reviewer approved all three objectives and the root requirement-by-requirement audit added one last TM corrupt-payload regression before reviewer reconfirmation. The sccache loopback limitation remains environment evidence only; it did not change project defaults or validation outcomes.
- **2026-08-21 — Completion events belong to the completing worker.** The reporter is synchronized and is already the sole terminal owner, so a publication worker emits `finished()` as soon as its caught execution result is ready. Result aggregation may remain in `JoinSet` order, but UI lifecycle state must not depend on when the scheduler drains that result.

## Completion Evidence

Planning evidence only; implementation evidence will be appended here as tasks complete.

### M1 implementation evidence

- Shared deterministic fixture: `tests/support/ncm_fixture.rs` carries the existing 1,056-byte upstream NCM parser header, a 1x1 valid PNG, and the repository silent-MP3 fixture. The release bench builds a 32 MiB default (configurable `UNLOCKM_NCM_BENCH_BYTES`, minimum 64 KiB) valid-MP3 payload with one header and repeated MPEG frames and checks the full SHA-256 of the recovered MPEG payload. Historical M1 validation used the then-pinned external transform; T2.2 replaced that builder call with an independent static stream vector. `UNLOCKM_NCM_BENCH_BYTES=1048576 rtk cargo bench --bench metadata_pipeline` passed: success, invalid-cover best effort, existing-output skip, and passthrough copy all reported payload SHA-256 `433c49d2fd67880643d28d89a12ccda847c8c3a1812fd5441e6ad0523dd84cfa`; each NCM run used jobs=1, 1 MiB buffer, and `update_metadata=true`; passthrough used `update_metadata=false`.
- Historical M1 reporting used coarse `preflight` (classification plus the then-heavy decrypt/metadata work) and `publication` phases. M3 later split the pipeline and current reports expose probe, decrypt, metadata resolve, tag write, fallback, copy, and publish. Default `RunnerMetrics` retains only O(1) aggregate files/logical-bytes/outcomes/timings; each transient per-path report is emitted immediately at trace, and a bounded `Vec` collector exists only under `cfg(test)`. `rtk cargo test app::runner::` initially passed 3 tests; the review-repair run passed 5 after adding invalid-NCM and flat-collision report coverage.
- Full M1 fresh-output local corpus baseline, 2026-08-21: release binary `c831a6d5b192bea5d8ec07d16701773c250a5706` + uncommitted M1 changes, SHA-256 `72c2f1e5d3bd14281cc4fd692f33b4b1ab2cde76e9d97268000e759c9815001e`; input `/Users/peilin/Music/网易云音乐`, 326 files, `3,577,484 KiB` apparent disk usage (`3,663,343,616` bytes), `/private/tmp` output roots on the same APFS volume. Every run used `--update-metadata --log-level warn --jobs N`, default 1 MiB buffer and metadata-jobs=4, converted=80/copied=246/skipped=0/conflicted=0/failed=0. There were no WARN lines and no source mutation; provider network request count is not instrumented in this pre-M3 build and is therefore unobserved rather than claimed zero. Each explicitly named `mktemp -d /private/tmp/unlockm-m1-*` output root was deleted after its result was recorded.

  | jobs | wall trials s | wall median s | user median s | sys median s | peak RSS median B |
  | ---: | --- | ---: | ---: | ---: | ---: |
  | 1 | 13.02, 11.68, 13.64 | 13.02 | 1.15 | 3.32 | 797,181,320 |
  | 2 | 6.95, 12.73, 10.45 | 10.45 | 1.28 | 4.04 | 716,391,648 |
  | 4 | 11.33, 6.77, 12.39 | 11.33 | 1.27 | 4.25 | 573,637,688 |
  | 8 | 13.42, 10.54, 13.70 | 13.42 | 1.31 | 4.84 | 797,197,632 |

  The uncached first pass was not counterbalanced against Go and results remain noisy, but the required three fresh-output Rust trials are now complete. Jobs=2 has the lowest current median; no default changes are made until the D8 cross-binary counterbalanced matrix after M3.
- `rtk cargo bench --bench metadata_pipeline` passed at the default 32 MiB payload: metadata success 0.066 s, invalid-cover best effort 0.037 s, existing-output skip 0.058 s, and passthrough 0.008 s; every recovered payload hash was `d402cfcac99dd1399ad7a9e9515f01a7758c986a5c554a08dafa9a9a43dd6eaa`. `rtk cargo bench --bench throughput` passed its 64 MiB QMC baseline: direct 1262.67 MiB/s, pipeline jobs=1 1221.03 MiB/s, 8-file jobs=1 1188.41 MiB/s, and current default jobs=8 4332.06 MiB/s.
- M1 review-repair validation: `rtk cargo fmt --all -- --check`; `rtk cargo test app::runner::` (5 passed, including default-no-collector, invalid NCM preflight Failed report, and two flat collision Conflict reports); `rtk cargo test --test failure_isolation` (1); `--test batch_layout` (6); `--test metadata_staging` (2); `--test logging` (3); `rtk cargo bench --bench metadata_pipeline` default 32 MiB (success 0.063 s, invalid-cover best effort 0.031 s, existing skip 0.059 s, passthrough 0.010 s, all payload hashes unchanged); and `rtk git diff --check` all passed. The collector is `cfg(test)` with an explicit report limit, so ordinary runs retain only scalar timing totals and no per-path `Vec`.
- M2/T2.1 validation: `rtk cargo test crypto::audio::` (5) and `rtk cargo test formats::audio::` (1) passed; the coverage includes multiple/large ID3 prefixes, fixed FLAC/Ogg/MP3/M4A fixtures, MP4/M4A magic, and junk/invalid-MP3 negatives. `rtk cargo test formats::` (10) and `rtk cargo test --test batch_layout` (6) also passed.
- M2/T2.2 validation: `rtk cargo test formats::` (11 after local NCM vector migration), `rtk cargo test --test metadata_staging` (2), `--test decrypt_fixtures` (3), and `--test no_network_by_default` (1) passed. `rtk cargo bench --bench metadata_pipeline` passed at its default 32 MiB: success 0.077 s, invalid-cover best effort 0.039 s, existing skip 0.063 s, passthrough 0.013 s, all recovered payload hashes `d402cfcac99dd1399ad7a9e9515f01a7758c986a5c554a08dafa9a9a43dd6eaa`. Source/test/bench audit found `umc_ncm` only in attribution comments, not executable references.
- M2/T2.3 validation: `rtk cargo test crypto::qmc::` passed 5 local v1-boundary, v2-map, v2-RC4, EKey, and Android footer tests; `rtk cargo test formats::` passed 11 including the retained 720-byte PC footer/EKey bounded-decrypt vector and QTag/STag metadata hints; `rtk cargo test --test batch_layout --test no_network_by_default --test decrypt_fixtures` passed 10 across 3 suites. A post-migration 8 MiB/4-file throughput smoke passed payload hashes: direct QMCv1 999.01 MiB/s, jobs=1 pipeline 869.97 MiB/s, many-file jobs=1 732.97 MiB/s, and jobs=8 1886.55 MiB/s. `src/`, `tests/`, and `benches/` have no executable `umc_qmc` references.
- M2/T2.4 validation: `rtk cargo test formats::` passed 11, including the synthetic KGM v5 header/self-test and bounded decrypt using only an audio-hash database EKey; `rtk cargo test keys::` passed 4, covering explicit QMC sources plus KGM database-path behavior. The local KGM database decrypt compiles against direct `aes`/`cbc`/`block-padding`/`byteorder`/MD5 helpers, and source search finds no executable `umc_kgm` callsite.
- M2/T2.5 validation: initial `rtk cargo test crypto::` passed 16 local parity tests, including pinned X2M/X3M 1 KiB header pairs, Kuwo KWMv1 header/payload round trip, Xiami header/round-trip transform, and QMC/NCM/audio vectors; `rtk cargo test formats::` passed 11; `rtk cargo test --test decrypt_fixtures --test batch_layout` passed 9 across 2 suites. Review repairs expanded local crypto/header/footer coverage to 24 tests before the final full matrix. The registry still rejects corrupt registered inputs and resolves shared `.xm` by positive Xiami validation before Ximalaya.
- M2/T2.6 validation: `rtk cargo check` and `rtk cargo clippy --all-targets --all-features -- -D warnings` passed after pruning unused copied APIs; the initial full matrix passed 94 tests and the review-repair final `rtk cargo test --all-targets --all-features` passed 106 tests across 13 suites; `rtk cargo build --release --locked` passed after each phase. `rtk cargo tree --locked --duplicates` reports only normal crates.io version duplicates. Searches of `Cargo.toml`/`Cargo.lock` find no git URL, `um_audio`, or `umc_*` package; executable-path search of `src/`, `tests/`, and `benches/` finds only source-attribution comments.
- M2 review-repair evidence: `rtk cargo test crypto::kgm:: -- --nocapture` passed 5 tests (pinned v2/v3 headers and chunk boundaries, invalid magic, three exact KDF key/IV vectors, encrypted page round trip, and synthetic encrypted page-1 restoration); `rtk cargo test crypto::qmc::` passed 9; focused KWMv2 and Ximalaya PC tests passed; `rtk cargo test formats::` passed 11 including PCv2 MID metadata hints and padded/unpadded/EncV2 EKey behavior. Final fmt, strict Clippy, 106-test full matrix, locked release build, dependency search, architecture search, and `git diff --check` passed.
- M2 independent closing review: source parity/dependency/notice/architecture audit passed; crypto tests (24 at review snapshot), formats tests (11), escalated full matrix, and strict Clippy passed. A locked release build had passed independently before the cfg(test)-only repair vectors; the reviewer's post-repair build rerun hit an intermittent configured-sccache `Operation not permitted` environment gap, which remains recorded rather than promoted to a pass.
- M3 correctness evidence: `rtk cargo test formats::` passed 12 including write-only-stage planned extension (proves no completed-stage read); app tests passed 12 including no-stage preflight, collision claims, clean-stage regeneration, source change, and cancellation; metadata tag tests passed 6 including in-place post-save failure and streaming FLAC growth/padding with unchanged audio bytes. Batch (6), metadata staging (2), metadata round-trip (4), failure/no-network/watch-focused tests remained green. Strict Clippy passed through the direct-rustc sandbox fallback while the configured sccache loopback remained intermittently unavailable to Cargo.

### M3 performance evidence

- Final release corpus: 326 files / `3,663,343,616` bytes, 80 converted + 246 copied + no failures for every fresh-output run; 1 MiB buffer, metadata-jobs=4, same APFS `/private/tmp` volume, `--log-level error` for the counterbalanced timing matrix. All explicitly created output roots were removed after result verification.

  | jobs | final wall trials s | wall median s | user median s | sys median s |
  | ---: | --- | ---: | ---: | ---: |
  | 1 | 4.39, 3.62, 3.50 | 3.62 | 0.93 | 1.33 |
  | 2 | 2.96, 2.99, 3.16 | 2.99 | 0.95 | 1.56 |
  | 4 | 3.32, 3.74, 3.39 | 3.39 | 0.98 | 1.68 |
  | 8 | 5.93, 5.66, 5.34 | 5.66 | 0.98 | 1.75 |

- Fixed Go `d0d3647...` reference, same corpus/fresh APFS outputs and metadata mode: 8.10/7.17/7.76 s wall, median 7.76 s; median user 4.06 s and sys 2.39 s. Final default Rust median is 0.385x Go and 70.0% faster than the original 9.97 s Rust baseline. Rust sys median is 68.7% below the original 4.98 s. The D8 wall/sys gates pass with wide margin.
- Escalated `/usr/bin/time -l` at final default jobs=2: 5.32 s wall / 0.99 user / 1.97 sys, maximum RSS `39,124,992` B and peak footprint `31,277,632` B. This is below the `2 * 1 MiB + 128 MiB` budget. Before the streaming FLAC writer the same measurement was `460,800,000` B RSS; no-metadata jobs=2 was `17,858,560` B RSS.
- No-metadata fresh corpus jobs=2 completed in 2.57 s (0.93 user / 1.12 sys). The final 64 MiB QMC benchmark reported direct 1272.18 MiB/s and one-file pipeline 1263.43 MiB/s versus M1 direct 1262.67 and pipeline 1221.03 MiB/s, so the no-metadata/direct-cipher 5% regression shield passes.
- Final M3 validation: formatting and `git diff --check` passed; strict Clippy passed with direct-rustc sandbox fallback; the elevated full `cargo test --all-targets --all-features` matrix passed 110 tests across 13 suites plus both benchmark executables. The configured sccache server remains the default, but Cargo-to-sccache loopback is intermittently denied by the sandbox; this environment-only rerun gap stays visible until a later default-wrapper validation succeeds.
- M3 blocker-repair validation: `staged_decrypt_never_publishes_after_cancel_or_source_change` constructs a real private decrypt stage, then independently cancels or mutates its source and proves the pre-existing destination remains byte-identical with no owned stage leak. `running_source_change_cancels_stale_job_and_requeue_resets_token` proves the watch state cancels a dirty running token and creates a fresh token for the single follow-up. Serialized `watch_recursive` and `watch_stability` suites passed 4 tests after the change.

### M4 presentation evidence

- `rtk cargo test presentation::` initially passed 3 and the review-repair run passed 4 unit tests, adding explicit TTY/TERM-dumb/NO_COLOR/JSON/always policy coverage. `rtk cargo test --test progress_output` passed 6 tests, including no cursor controls for pre-start config failure; logging and CLI suites add 5 more passing compatibility tests. Coverage includes redirected auto/never stable summary with no controls, forced two-row output and narrow Unicode ellipsis, progress-aware warning/partial result, JSON-only parseable summary, JSON/always conflict, help, exit codes, and logging compatibility.
- Manual PTY batch/watch smoke used `--progress always`: the initial `0/1` bar advanced exactly to `1/1 100.0%`, then changed to `Watching processed=1 active=0`; a Unicode `新曲.lrc` event rendered `Planning` then `Copying` and returned to `Waiting`. An external SIGINT produced cursor restoration, friendly `Done in ...` summary, exit 0, and no owned temp. The explicitly created `/private/tmp/unlockm-watch-ui.*` root was removed.
- Help now documents every flag and shows `[default: 2]` jobs plus progress values. Redirected human output retains the original `unlockm: converted=...` line; interactive output uses `Done in ... — ...`; JSON ends with `{event:"summary", elapsed_ms, counts, exit_code}` and no live ANSI.
- The post-review 326-file live-progress defect is covered by `completed_files_advance_before_the_next_serial_job_starts`: with `--jobs 1`, captured output must render `1/3` before at least one later `Copying:` start. The old result-drain behavior fails this ordering assertion. The repaired worker completion event passed all 7 `progress_output` tests and all 5 runner tests before the final full matrix.

### M5 final validation evidence

- Final `rtk cargo fmt --all -- --check`, direct-rustc fallback `cargo clippy --all-targets --all-features -- -D warnings`, and direct-rustc fallback `cargo build --release --locked` passed. After the live-progress repair, elevated direct-rustc `cargo test --all-targets --all-features` passed 124 tests across the library/main and 12 integration suites, then both benchmark executables completed with payload hashes. `rtk git diff --check` passed.
- `README.md` was rewritten as the user-facing English guide and links to the matching `README.zh.md` translation on its first content line; the Chinese guide links back. Both explain two-file concurrency, progress/output modes, formats and keys, metadata privacy, watch mode, exit status, troubleshooting, and safety using corresponding sections and commands.
- `cargo tree --locked --duplicates` completed through the same fallback and reports only normal crates.io duplicates (`getrandom` and `syn`). Manifest/lock search has no `lib_um_crypto_rust` git URL and no `um_audio`/`umc_*` package. Source/test/bench matches are attribution comments only. QTFM is absent.
- Final release NCM smoke used the local `../samples/Claire Rosinkranz - Frankenstein.ncm`. With and without `--update-metadata`, both runs returned `converted=1, failed=0` and produced 16-bit stereo 44.1 kHz FLAC with 8,919,168 samples. The metadata result contains `TITLE=Frankenstein` and `ALBUM=Frankenstein`; both FLAC STREAMINFO MD5 values are `5b9f96835b661a0bf8a865e05d4b94cd`, proving identical decoded audio. Full container SHA-256 differs as expected (`e9e53c...aa68` tagged versus `a0fd78...7429` plain). Temporary smoke roots were removed.
- The repository/default environment still configures `/opt/homebrew/bin/sccache`; its server was restarted and directly answers `sccache rustc -vV`, but sandboxed Cargo-to-server loopback intermittently returns `Operation not permitted`. Required final validation therefore used a one-command `RUSTC_WRAPPER=` fallback without modifying project/user defaults or clearing the cache. This remains the only local environment validation note; code/tests/build themselves are green.
- Repeatable final comparison protocol: build locked release binaries for Rust and Go from recorded commits; warm source cache once; create fresh same-APFS output roots; alternate Go/Rust order for at least three measured runs per jobs count; run `/usr/bin/time -l <binary> /Users/peilin/Music/网易云音乐 -o <fresh-output> --update-metadata --jobs <1|2|4|8>` for Rust and the equivalent `./um -i ... -o ... --update-metadata` for Go; record corpus counts/bytes, binary hash, warning/network count, wall/user/sys, peak RSS, stage bytes, and output integrity; remove only the explicitly created temporary roots after each verified run.

- 2026-08-21 user baseline: Rust 9.97 s wall / 1.34 s user / 4.98 s sys; Go 7.15 s wall / 3.69 s user / 2.75 s sys on the same reported NCM directory with metadata update.
- 2026-08-21 source inspection: Rust performs bounded classification, full staged decrypt, completed-output sniff reread, Lofty read, NCM reparse, full stage-to-candidate copy, Lofty save/re-read, a global heavy preflight barrier, then copy/publish. Go processes sequentially, reuses its NCM decoder metadata, writes one decrypted temp, and saves final metadata without Rust's stage-preservation transaction.
- No production code, tests, manifests, or configuration were changed during the Plan phase.

### Outcomes & Retrospective

The implementation delivers all three requested outcomes as one coherent pipeline change: responsive two-row progress and stable output modes; a crate-private, fully local crypto/audio implementation with no external git source; and a metadata-heavy execution path that is both substantially faster and bounded-memory. The key performance insight was that cipher CPU was not the bottleneck: full-stage lifecycle, candidate copying, Lofty's full-file FLAC writer, and excessive IO concurrency dominated wall/sys/RSS. Bounded prefix planning, in-place private-stage recovery, a streaming FLAC block writer, early skip, `io::copy`, and a two-job real-IO default addressed those costs without weakening atomic publication or source retention.

The final default Rust corpus median is 2.99 s versus Go's 7.76 s, maximum metadata RSS is 39.1 MB, and decoded audio identity is preserved. Independent review cycles found and closed bounded-report memory, missing crypto vectors, stale-stage publication, and NO_COLOR issues. The sole environment note is sandbox denial of Cargo-to-sccache loopback; default/user cache configuration remains unchanged and all required commands passed through a one-command direct-rustc fallback.

### Review Notes

- [x] M1: default report retention changed from unbounded per-path storage to O(1) aggregates; failure/conflict report vectors and fixture-dependency wording independently re-reviewed.
- [x] M2: missing KGM/QMC/Kuwo/Ximalaya parity vectors and stale architecture symbol repaired; dependency independence and source parity independently re-reviewed.
- [x] M3: final cancellation/source-fingerprint gate and dirty-running watch cancellation repaired; stale output publication Blocker independently re-reviewed.
- [x] M4: Auto progress now honors `NO_COLOR`; cursor/JSON/output policy independently re-reviewed with no remaining finding.
- [x] M5 final independent review passed; the post-review TM fail-closed delta was independently reconfirmed and no finding remains open.
- [x] Post-review progress follow-up: real-run event-timing defect reproduced from the scheduler, repaired without changing conversion concurrency, and covered by ordering-sensitive integration plus full release validation.
