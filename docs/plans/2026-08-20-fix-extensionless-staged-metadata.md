# Fix Extensionless Staged Metadata Handling

## Status

Done

Created: 2026-08-20
Last Updated: 2026-08-21 — independent review passed with all local findings closed; root final validation completed.

## Objective

Make `--update-metadata` work on decrypted audio while it is still stored under an owned extensionless `.unlockm-stage-*` path. A valid NCM/QMC conversion must retain or enrich native tags and artwork without emitting the observed `No format could be determined from the provided file` warnings merely because the private staging filename has no suffix.

The user-visible result is that a command such as:

`unlockm <input> -o <output> --update-metadata`

converts valid audio, applies eligible metadata, and stays quiet at the default `warn` level. A genuine metadata/provider/tag failure remains best effort: the audio conversion succeeds and emits one actionable warning for the affected source file.

## Scope

- Make native tag reads, writes, and post-write verification identify MP3, FLAC, M4A/MP4, and Ogg from content rather than the staging path extension.
- Preserve existing tag/artwork provenance, merge rules, cover validation, HTTPS restrictions, and audio-payload integrity checks.
- Preserve the existing extensionless owned staging name, same-filesystem publication, destination extension selection, collision handling, overwrite recovery, cancellation cleanup, and watch filtering.
- Consolidate metadata diagnostics so one failed enrichment attempt produces one source-oriented warning instead of both an internal tag-read warning and an outer `metadata update skipped` warning.
- Add deterministic extensionless tag regressions and an end-to-end synthetic NCM conversion that exercises the real `stage_decrypt -> enrich_staged -> publish` path with `--update-metadata`.
- Document the corrected behavior and useful `--log-level` troubleshooting commands.

## Non-Goals

- Changing CLI flags, defaults, exit codes, `--log-level` parsing, or JSON log formatting.
- Renaming staging files to include their detected output extension.
- Changing NCM/QMC provider matching, metadata precedence, artwork precedence, or outbound-origin policy.
- Adding new audio containers, encrypted formats, transcoders, or an FFmpeg dependency.
- Reworking atomic publication, batch scheduling, watch state, or key discovery except for regression coverage proving they remain intact.
- Retagging files already produced by an earlier affected run. Users can rerun into a fresh output or use `--overwrite` after this fix is released.

## Current Context

- `src/app/job.rs::stage_decrypt` creates a same-output-filesystem tempfile with prefix `.unlockm-stage-`, decrypts into it, learns the native extension from payload sniffing, and invokes `metadata::enrich_staged` before final destination planning/publication.
- The detected extension is currently returned separately; it is not present in `StagedDecrypt::path` when metadata runs. This is intentional for owned-temp cleanup and is not itself an audio-format error.
- `src/metadata/tags.rs::read` and `write` call `lofty::read_from_path`. Lofty 0.21 documents that `read_from_path` determines `FileType` from the extension, so it cannot classify `.unlockm-stage-*` even when the bytes contain valid MP3/FLAC/M4A/Ogg audio.
- `src/metadata.rs::enrich_staged` logs `could not read tags`, falls back to empty metadata, and may then call `tags::write`; the propagated write failure is logged again by `src/app/job.rs` as `metadata update skipped`.
- The captured real run converted 80 encrypted files with `failed=0`, but produced exactly 80 tag-read warnings plus 80 metadata-skip warnings. Therefore decryption/publication succeeded while metadata enrichment was skipped for every converted file.
- Lofty 0.21 also provides `lofty::read_from(&mut File)`, which performs content guessing, while `AudioFile::save_to` writes a previously identified `TaggedFile` to an opened file without needing to infer type from the path.
- `tests/metadata_roundtrip.rs` already contains licensed deterministic MP3, FLAC, M4A, and Ogg fixtures plus audio-payload fingerprint helpers. `src/formats.rs` contains an upstream NCM header/metadata vector that can be combined in tests with a licensed standard-audio payload rather than committing copyrighted music.
- `src/io/atomic.rs`, `src/app/runner.rs`, and watch integration tests already own staging cleanup, collision grouping, cancellation, and publication behavior. The fix must continue to use these paths unchanged.
- The completed initial implementation plan is `docs/plans/2026-08-20-initial-rust-unlockm-cli.md`; this focused follow-up plan records the production regression without reopening or rewriting that historical plan.

## Design and Decisions

### Content-based tag loading

Centralize Lofty loading in `src/metadata/tags.rs` behind an internal helper that opens the staged file and calls the content-probing API (`lofty::read_from`) instead of the extension-based `lofty::read_from_path`. Both public `read` and `write` use this helper, including the post-write round-trip verification.

For writes, retain the detected `TaggedFile`/file type, mutate its tag, open the same staging path read-write, and save through `AudioFile::save_to` (or an equivalently extension-independent Lofty API). Do not derive a tag format from the encrypted source suffix or from the expected destination suffix; the decrypted bytes remain the authority.

### Keep staging ownership unchanged

Do not rename `.unlockm-stage-*` after decryption. Renaming to append `.mp3`/`.flac` would add an unnecessary filesystem transition and couple tag support to path mutation. Content probing fixes the adapter at its proper boundary while preserving hidden-temp recognition, `StagedDecrypt::Drop`, watch exclusion, flat preflight grouping, and `AtomicOutput::publish_existing_temp`.

### Conservative failure and single warning ownership

`metadata::enrich_staged` must return contextual errors rather than emit its own warning and continue from an unreadable tag state. A valid file with no tags still returns an empty `Metadata` value normally. If the container cannot be read safely, skip enrichment rather than assuming existing tags are absent.

`stage_decrypt` remains the warning owner and logs one `metadata update skipped` event with the original encrypted source path and contextual error. Provider, cover, tag-write, and verification failures remain warning-only and do not increment the conversion failure count. No secrets, provider payloads, or raw keys may enter diagnostics.

### Regression fixture strategy

Extend the real-media tag suite so each supported fixture is copied to an extensionless `.unlockm-stage-*` path, then read, written, and re-read through the production tag adapter. Assert text, cover bytes, decoder-derived audio properties, and container-aware audio-payload fingerprints.

Add a CLI-level metadata staging test that builds a deterministic NCM input from the existing licensed NCM header/key/metadata vector and a licensed silent standard-audio fixture. Encrypt the payload with the same offset-aware symmetric NCM transform used by the pinned crate, run `unlockm --update-metadata` into a fresh temporary output, and assert:

- exit status is successful with `converted=1` and `failed=0`;
- the final native extension is correct and no owned stage remains;
- container title/album/artist and eligible embedded artwork round-trip from the published output;
- the audio payload remains unchanged apart from the tag container;
- stderr contains neither `could not read tags`, `metadata update skipped`, nor `No format could be determined`.

Add a separate genuine tag-failure case to prove best-effort conversion and exactly one source-oriented warning. Do not weaken warning assertions by globally selecting `--log-level error` or `off` in the regression.

## Milestones

### M1. Extension-independent tag adapter

Goal: make the metadata boundary classify staged audio from bytes and fail conservatively with a single caller-owned diagnostic.

Dependencies: existing Lofty 0.21 metadata adapter and real-media fixtures.

Acceptance criteria:

- `tags::read`, `tags::write`, and post-write verification work on valid extensionless MP3, FLAC, M4A, and Ogg files.
- Existing text/artwork and audio-payload integrity behavior is unchanged on normal extension-bearing paths.
- An unreadable or unsupported tag container returns a contextual error before metadata merge/write assumptions are made.
- No internal metadata layer emits a duplicate warning for a failure already reported by the job boundary.

Validation:

- `rtk cargo test metadata::tags::`
- `rtk cargo test --test metadata_roundtrip`
- `rtk cargo test --test logging`

### M2. Real staging-pipeline regression

Goal: prove that `--update-metadata` works through the same extensionless staging and atomic publication pipeline used by batch and watch processing.

Dependencies: M1 and the existing NCM/Lofty licensed vectors.

Acceptance criteria:

- A deterministic synthetic NCM carrying valid standard audio converts and receives its container metadata under `--update-metadata`.
- The successful case emits no metadata-format warning at default `warn` level.
- A genuine metadata failure emits exactly one warning, still publishes valid decrypted audio, and leaves `failed=0`.
- Destination extension selection, stage cleanup, collision semantics, and no-network-without-flag behavior retain regression coverage.

Validation:

- `rtk cargo test --test metadata_staging`
- `rtk cargo test --test no_network_by_default`
- `rtk cargo test --test batch_layout`
- `rtk cargo test io::atomic::tests::`

### M3. Documentation and release validation

Goal: reconcile user guidance and complete local cross-target validation without overstating remote CI evidence.

Dependencies: M1-M2.

Acceptance criteria:

- `README.md` explains that `--update-metadata` operates on content-probed staging files and documents `error`, `warn`, `info`, `debug`, `trace`, and `off` usage accurately.
- `ARCHITECTURE.md` records that metadata adapters must not depend on private staging filename extensions.
- Formatting, warnings-denied Clippy, all tests/features, and the locked release build pass.
- A fresh-output release smoke using the available local NCM sample completes without the two regression warning messages; its output remains probeable and its metadata can be read from the final extension-bearing file.

Validation:

- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy --all-targets --all-features -- -D warnings`
- `rtk cargo test --all-targets --all-features`
- `rtk cargo build --release --locked`
- `rtk cargo run --release -- --no-copy --update-metadata --log-level warn -o <fresh-temporary-output> '../samples/Claire Rosinkranz - Frankenstein.ncm'`
- Probe the smoke output with `rtk file` and read its tags through the production tag adapter or another available native inspector.

## Task Breakdown

### M1 tasks

- [x] T1.1 Add extensionless-path regression cases for MP3, FLAC, M4A, and Ogg in `tests/metadata_roundtrip.rs`; demonstrate the current `read_from_path` failure before changing production code, then verify text, artwork, properties, and payload hashes after the fix.
- [x] T1.2 Replace extension-based Lofty loading in `src/metadata/tags.rs` with one content-probing helper used by read, write, save, and post-write verification; verify `rtk cargo test --test metadata_roundtrip`.
- [x] T1.3 Refactor `src/metadata.rs::enrich_staged` and `src/app/job.rs::stage_decrypt` so unreadable-tag errors retain stage/source context and one job-level warning owns diagnostics; add assertions that valid no-tag audio is not treated as an error.

### M2 tasks

- [x] T2.1 Add a self-contained licensed synthetic NCM builder and CLI regression in `tests/metadata_staging.rs`, exercising extension sniffing, extensionless enrichment, final publication, tag/artwork round-trip, payload integrity, and owned-stage cleanup.
- [x] T2.2 Add a genuine metadata-adapter failure case in `tests/metadata_staging.rs` or `tests/logging.rs`; assert conversion success, `failed=0`, exactly one `metadata update skipped` warning for the source, and no duplicate `could not read tags` event.
- [x] T2.3 Re-run `tests/no_network_by_default.rs`, batch collision/stage-cleanup tests, and atomic publication tests to prove the metadata fix did not create a second pipeline or alter output safety.

### M3 tasks

- [x] T3.1 Update `README.md` troubleshooting/log-level guidance and the metadata invariant in `ARCHITECTURE.md`; do not imply `--json-logs` changes filtering or that `RUST_LOG` overrides the explicit CLI filter.
- [x] T3.2 Run the complete local validation matrix and a fresh-output release NCM smoke, record exact commands/results under Completion Evidence, and move this plan to `In Review` for independent review.

## Validation Strategy

### Focused red/green checks

- First make the regression observable with a valid audio fixture named only `.unlockm-stage-<id>`; the pre-fix test must fail specifically because Lofty cannot determine a format from the extensionless path.
- Run metadata tag tests after every adapter change. Test all four currently claimed native tag containers rather than assuming the FLAC incident generalizes safely.
- Capture CLI stderr in integration tests and count structured message substrings. A successful update must produce zero regression warnings; an intentionally unsupported update must produce exactly one job-level warning.

### Correctness and safety checks

- Compare tags and picture bytes after re-read.
- Compare decoder-derived duration/sample-rate/bit-depth/channel data and existing container-aware audio payload hashes before and after metadata writes.
- Assert successful and failed enrichment paths remove all `.unlockm-stage-*` and `.unlockm-backup-*` artifacts.
- Assert a pre-existing destination is unchanged when overwrite is not requested and retains atomic recovery behavior when overwrite publication is injected to fail.
- Assert omitting `--update-metadata` still performs no provider request and does not invoke tag mutation.
- Exercise the new CLI regression through the shared runner so batch and watch inherit the same correction; retain serialized watch suites in the final matrix.

### Final commands

- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy --all-targets --all-features -- -D warnings`
- `rtk cargo test metadata::`
- `rtk cargo test --test metadata_roundtrip`
- `rtk cargo test --test metadata_staging`
- `rtk cargo test --test logging`
- `rtk cargo test --test no_network_by_default`
- `rtk cargo test --test batch_layout`
- `rtk cargo test --all-targets --all-features`
- `rtk cargo build --release --locked`
- Observe the configured Linux/macOS/Windows CI matrix after a later requested push; an unobserved remote run remains a validation gap.

## Risks and Mitigations

- **Content probing recognizes a different type than a misleading suffix previously selected.** The staging bytes are decrypted payload authority, and the detected `TaggedFile` type is retained through write. Cross-container extensionless fixtures guard this behavior.
- **Saving through an opened file may depend on cursor position or truncate semantics.** Use Lofty's `AudioFile::save_to` contract with a dedicated read-write handle and verify complete payload hashes, not just readable tags.
- **A tag error could partially mutate the private stage.** Retain existing non-destructive unsupported-container regressions and payload checks. Because publication is best effort after metadata failure, the implementer must verify that every introduced error path leaves the staged audio probeable; do not publish an empty or truncated stage.
- **Warning consolidation might hide useful detail.** Preserve the full contextual error chain in the single job-level event and keep the original encrypted source path. Only duplicate emission is removed.
- **The synthetic NCM fixture could accidentally depend on copyrighted content.** Combine the already attributed upstream NCM vector with the existing MIT/Apache-2.0 silent media fixtures; do not commit the user's music or decrypted output.
- **The local NCM smoke may contact an allowed artwork CDN.** Use embedded deterministic cover data in automated tests. Treat the release smoke as optional-network best effort and validate conversion/tag behavior independently of network availability.
- **Cross-platform Lofty or file-locking behavior may differ.** Close the decrypt writer before tag loading, avoid simultaneous stale handles during save, and rely on the existing Windows/macOS/Linux CI matrix. Do not claim unobserved remote CI as passed.

## Progress

- [x] 2026-08-20: Classified the supplied run log: 80 conversions succeeded, while all 80 metadata attempts emitted one tag-read warning and one outer metadata-skip warning.
- [x] 2026-08-20: Traced the failure to extensionless `.unlockm-stage-*` paths passed to Lofty 0.21's extension-based `read_from_path` in both tag read and write paths.
- [x] 2026-08-20: Inspected staging ownership, runner collision grouping, atomic publication, watch temp filtering, real-media fixtures, logging tests, and the completed initial ExecPlan.
- [x] 2026-08-20: Chose content-based Lofty probing plus single job-level warning ownership; no CLI/product decision remains unresolved.
- [x] 2026-08-20: Re-opened for independent review findings: vendor deterministic MP3 fixture, make staged tag mutation transactional, and reconcile historical plan state.
- [x] 2026-08-20: Began T1.1 after reading the active plan, architecture, CI guidance, current tag adapter, staging job, and real-media fixture suite.
- [x] 2026-08-20: Completed T1.1-T1.2. The extensionless regression first failed in the old adapter with `No format could be determined from the provided file`; `tags::probe` now opens the stage and uses `lofty::read_from`, while writes use `TaggedFile::save_to` on an opened read-write handle. MP3, FLAC, M4A, and Ogg extensionless stages now round-trip text/artwork, decoder audio properties, and payload hashes. `rtk proxy cargo test --test metadata_roundtrip` passed (4 tests).
- [x] 2026-08-20: Completed T1.3. `enrich_staged` now returns contextual stage read/write errors without logging; `stage_decrypt` remains the sole warning owner with the original source path. `valid_tagless_audio_is_an_empty_metadata_value` proves a content-probed extensionless but tagless MP3 remains a normal empty `Metadata` result. `rtk proxy cargo test metadata::tags::` passed (2 tests); existing logging and round-trip suites passed.
- [x] 2026-08-20: Completed T2.1-T2.3. `tests/metadata_staging.rs` has a self-contained base64 NCM parser vector combined with the licensed silent MP3, a valid embedded PNG, and NCM's symmetric stream transform. The CLI regression proves the shared staging/publish pipeline produces `synthetic.mp3` with valid art, unchanged MPEG payload, zero stage leftovers, and zero regression warnings. Its invalid-art sibling proves successful publication with `failed=0`, exactly one source-oriented metadata warning, and no tag-read warning. `rtk proxy cargo test --test metadata_staging` passed (2 tests); no-network (1), batch layout (6), and atomic (5) regressions passed.
- [x] 2026-08-20: Completed T3.1-T3.2. README documents content-probed private stages and all supported explicit log levels; ARCHITECTURE records that tag adapters must not use staging filename extensions. Formatting, warnings-denied Clippy, all targets/features, locked release build, and a fresh local release NCM smoke completed; this plan is `In Review` pending independent review and remote CI evidence.

## Discoveries

- The real run's `converted=80, failed=0` summary proves this is not a decryption or output-publication failure. The user received playable outputs, but `--update-metadata` did not complete for any of those 80 files.
- Historical, fixed: the original warning volume was doubled by an internal metadata warning plus the job warning. The lower layer now returns contextual errors and the job owns the one user-facing warning.
- Historical, fixed: Lofty 0.21's `read_from_path` determines type from the path extension, whereas `read_from(&mut File)` invokes `Probe::guess_file_type` on content. The current adapter uses content probing and `save_to` on an opened file.
- The final output extension is already detected correctly by `um_audio` before metadata runs; the bug is confined to the native tag adapter's independent format detection.
- The current tag round-trip fixtures all use normal extensions, so they validate container correctness but cannot catch extensionless production staging regressions.
- The pre-fix extensionless MP3 regression failed exactly at `tags::write` with Lofty's `No format could be determined from the provided file`; retaining the direct `read_from_path` assertion in the green regression documents why content probing is required.
- The initial local smoke invocation used a stale `target/release/unlockm` built before this change and reproduced the old warnings; rebuilding with `rtk proxy cargo build --release --locked` and rerunning was clean. The stale-binary run is not validation evidence for the current source.

## Decision Log

- **2026-08-20 — Use content probing at the metadata boundary.** Decrypted bytes, not private path names or encrypted source suffixes, determine the Lofty file type.
- **2026-08-20 — Keep extensionless staging paths.** This avoids extra renames and preserves existing cleanup, watch exclusion, collision, cancellation, and atomic publication behavior.
- **2026-08-20 — Fail conservatively on unreadable existing tags.** A valid tagless file is distinct from an unreadable container; do not assume missing metadata when the existing state cannot be inspected.
- **2026-08-20 — Emit one warning per failed enrichment attempt.** The job boundary owns user-facing warnings and includes the original source path; lower metadata layers return contextual errors.
- **2026-08-20 — Use only licensed synthetic fixtures in CI.** The available real NCM sample remains a local smoke input and is never committed or redistributed.
- **2026-08-20 — Transactional staged tag mutation.** Metadata writes copy the private stage to a same-directory owned candidate, save and verify only that candidate, then publish it over the stage through `AtomicOutput`'s recoverable overwrite path. A failed save or verification deletes the candidate and leaves the original decrypted stage eligible for successful publication.
- **2026-08-21 — Bounded tag verification and fault isolation.** Production tag verification uses metadata length, content probing, and tag round-trip checks only; payload hashing remains test-only. Fault injection is a per-call closure, never process-global state, and early atomic publication errors clean the owned candidate.

## Completion Evidence

Implementation evidence:

- 2026-08-20: Pre-fix T1.1 red check: `rtk proxy cargo test --test metadata_roundtrip lofty_real_media_round_trips_when_staged_without_an_extension -- --nocapture` failed at `tags::write` with `No format could be determined from the provided file` for the extensionless MP3 stage.
- 2026-08-20: T1 focused green checks: `rtk proxy cargo test metadata::tags::` passed (2); `rtk proxy cargo test --test metadata_roundtrip` passed (4); and `rtk proxy cargo test --test logging` passed (3).
- 2026-08-20: T2 staged-pipeline checks: `rtk proxy cargo test --test metadata_staging` passed (2); `rtk proxy cargo test --test no_network_by_default` passed (1); `rtk proxy cargo test --test batch_layout` passed (6); and `rtk proxy cargo test io::atomic::tests::` passed (5).
- 2026-08-20: Final local matrix: `rtk cargo fmt --all -- --check` passed; `rtk proxy cargo clippy --all-targets --all-features -- -D warnings` passed; elevated `rtk proxy cargo test --all-targets --all-features` passed (48 unit tests, 27 integration tests, and the throughput executable); and `rtk proxy cargo build --release --locked` passed.
- 2026-08-20: Fresh-output release smoke: after the locked release rebuild, `rtk proxy /bin/zsh -lc 'task_dir=$(mktemp -d /private/tmp/unlockm-metadata-smoke-new.XXXXXX) && target/release/unlockm --no-copy --update-metadata --log-level warn -o "$task_dir" "../samples/Claire Rosinkranz - Frankenstein.ncm"'` completed with `converted=1, copied=0, skipped=0, conflicted=0, failed=0`, emitted neither regression warning, and published a 16-bit stereo 44.1 kHz FLAC. `rtk strings <output> | rtk rg -i 'title=|album=|artist='` found `TITLE=Frankenstein` and `ALBUM=Frankenstein`. The output is under `/private/tmp` and is not committed.
- 2026-08-20: Review-fix validation: `rtk proxy cargo test metadata::tags::` passed (3, including injected post-save failure); `rtk proxy cargo test --test metadata_roundtrip` passed (4); `rtk proxy cargo test --test metadata_staging` passed (2); `rtk cargo fmt --all -- --check` and `rtk proxy cargo clippy --all-targets --all-features -- -D warnings` passed; elevated `rtk proxy cargo test --all-targets --all-features` passed (49 unit tests, all integration suites, throughput executable); and `rtk proxy cargo build --release --locked` passed. Fresh local smoke to `/private/tmp/unlockm-metadata-fix-smoke.uv8M02` returned `converted=1, failed=0` with no metadata warning and a probeable 16-bit stereo 44.1 kHz FLAC.

Remaining external evidence:

- Configured Windows/Linux/macOS CI has not been observed from this local session; it remains a review/CI gap.

- 2026-08-21: Re-review-fix validation: `rtk proxy cargo test metadata::tags::` passed (3; per-call post-save fault preserves original bytes/probeability); `rtk proxy cargo test io::atomic::tests::` passed (6; injected early backup-rename failure preserves stage and removes candidate/backup); `rtk proxy cargo clippy --all-targets --all-features -- -D warnings` passed; elevated `rtk proxy cargo test --all-targets --all-features` passed (50 unit tests plus every integration suite and throughput executable); and `rtk proxy cargo build --release --locked` completed after the full matrix.
- 2026-08-21: Final V3 post-source-fix matrix: `rtk cargo fmt --all -- --check` passed; elevated `rtk proxy cargo clippy --all-targets --all-features -- -D warnings` passed; elevated `rtk proxy cargo test --all-targets --all-features` passed after the returned-candidate test correction (52 unit tests, every integration suite, and the throughput executable); and `rtk proxy cargo build --release --locked` completed in the same final matrix.
- 2026-08-21: Independent final review result was `Pass with Notes`; all eight Review Notes below were independently verified and closed. The reviewer observed 79 passing tests across 13 suites plus clean formatting, warnings-denied Clippy, and a locked release build. The only note is the explicitly unobserved remote Windows/Linux/macOS CI run.
- 2026-08-21: Root final audit passed `rtk cargo fmt --all -- --check`, `rtk cargo clippy --all-targets --all-features -- -D warnings`, elevated `rtk cargo test --all-targets --all-features` (79 passed across 13 suites), and `rtk cargo build --release --locked`. A fresh release smoke at `/private/tmp/unlockm-root-final-smoke.z5HpCp` completed with `converted=1, copied=0, skipped=0, conflicted=0, failed=0`, emitted no metadata warning, produced a 16-bit stereo 44.1 kHz FLAC, and exposed `TITLE=Frankenstein` plus `ALBUM=Frankenstein` through native string inspection.

## Outcomes & Retrospective

The extensionless-stage fix now keeps test fixtures self-contained and treats metadata mutation as a transaction: the conversion pipeline never publishes a stage altered by a failed tag save or verification. Independent review closed every local correctness, safety, performance, test, and plan finding. Configured remote CI remains unobserved external evidence rather than an open implementation finding; no release or commit was created in this implementation run.

Historical implementation checklist retained for context:

- the pre-fix failing extensionless regression and post-fix focused test counts;
- the synthetic NCM CLI result, final detected type, tag/artwork assertions, payload fingerprint, warning count, and owned-temp cleanup result;
- exact formatting, Clippy, full-test, locked-release-build, and local smoke outcomes;
- remote platform CI links/results when they are actually observed.

## Review Notes

- [x] Major (2026-08-20, independent review): `tests/fixtures/lofty-silent.mp3.b64` vendors the deterministic licensed MP3, all `ffmpeg_silent.mp3` sibling paths are removed from `src` and `tests`, and `THIRD_PARTY_NOTICES.md` records its Project Unlock Music MIT OR Apache-2.0 attribution. Re-reviewed: the fixture decodes to 733 bytes, all source/test path searches are clean, and focused plus full tests run without consuming a sibling fixture.
- [x] Major (2026-08-20, independent review): `tags::write` copies the private stage to same-directory `.unlockm-metadata-*`, mutates and round-trip verifies only that candidate, then uses `AtomicOutput::publish_existing_temp(..., overwrite=true)` to replace the stage. The injected post-save regression proves the original bytes remain probeable and unchanged while the candidate is cleaned; the CLI failure regression retains one source warning with successful publication. Re-reviewed for the normal post-save failure path.
- [x] Minor (2026-08-20, independent review): stale Progress/Discoveries claims are removed or marked historical, the current `save_to` behavior is documented, remote CI remains explicitly a gap, and `Outcomes & Retrospective` records the implemented result.
- [x] Major (2026-08-21, re-review): `write_candidate` no longer calls `fs::read`; production uses only `fs::metadata().len()`, bounded Lofty content probes, and tag round-trip verification. Payload hashes remain test-only. Re-reviewed against Lofty 0.21's `read_from(File)` implementation (a buffered seekable reader) and source searches.
- [x] Minor (2026-08-21, re-review): `publish_temp_with` cleans its owned temp on every error and `tags::write` retains caller-side cleanup around publication; an injected early backup-rename failure preserves the original stage and leaves no candidate/backup.
- [x] Minor (2026-08-21, re-review): the global fault flag is removed in favor of a per-call `write_with_hook` closure, so parallel tag writes cannot consume another test's injected failure.
- [x] Major (2026-08-21, re-review v2): `publish_temp_inner` now handles a failed backup restoration after final rename failure. A verified metadata candidate is promoted to `StagedDecrypt`'s active final-publish path, the original stays under cleanup ownership, and generic atomic publication restores by copy when both renames fail or reports the retained backup explicitly if fallback copy fails. Re-reviewed with the double-rename regression and final-pipeline lifecycle inspection.
- [x] Minor (2026-08-21, re-review v2): candidate copy failure deletes the candidate immediately; the regression uses a directory source to force copy failure and asserts no `.unlockm-metadata-*` remains.
