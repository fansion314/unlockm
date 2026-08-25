# ARCHITECTURE.md

## 1. Project Overview

`unlockm` is a cross-platform Rust conversion engine and CLI that turns encrypted files downloaded by music applications into standard audio files while preserving directory layout, companion files, metadata, and album art. `UnlockM` is its first desktop host: a Tauri application for macOS and Windows that links the same Rust engine directly.

The first release supports one-shot file/directory conversion and a long-running recursive watch mode. It targets Windows, macOS, and Linux, favors high aggregate throughput, keeps memory bounded, and never requires audio transcoding.

The repository implements the layout and invariants below. Its pinned behavioral/source references are:

- Go CLI `../cli` at `d0d3647d615c8624abf27e30ade07d23da757041`, especially its resilient recursive processing and passthrough behavior.
- Rust crypto workspace `../lib_um_crypto_rust` at `5dad838c33c3e4aab198d995160d3fbb9968470f` (`v0.1.12`) as the pinned source reference for the crate-private cipher and audio-sniffing primitives in `src/crypto/`.

Non-goals are audio transcoding, library-management features, automatic source deletion, a desktop watch mode, and writing metadata obtained from an ambiguous network search. The desktop host is intentionally not a wrapper that starts the CLI: it never launches `unlockm`, a sidecar, a shell command, or a conversion daemon. Explicit reveal/open controls are a separate user-triggered boundary: Rust resolves a persisted run/item ID, revalidates that recorded output, then asks the operating system to show it or open it with the default application.

## 2. High-Level Architecture

The CLI and desktop host are thin configuration/persistence boundaries. One shared application pipeline serves initial scans, direct file conversion, and watch events; desktop uses batch conversion only and observes the same bounded pipeline through a UI-neutral event seam.

```text
CLI/config ──┐
             ├─> scan or watch -> path planner -> bounded job runner
Tauri IPC ───┘                                      |             |
                                              format adapter    passthrough copy
                                                      |
                                              streaming decrypt
                                                      |
                                          conservative metadata merge
                                                      |
                                            atomic output publish
                                                      |
                                BatchObserver -> bounded desktop coordinator -> SQLite history -> IPC Channel
```

Tokio owns cancellation, filesystem events, network metadata requests, and task coordination. File scanning, decryption, copying, audio probing, and tag writes are blocking operations and run behind a concurrency semaphore on `spawn_blocking`. Each file is processed sequentially, while independent files run concurrently.

This hybrid model avoids blocking the async runtime without forcing synchronous cipher APIs into artificial async wrappers. It also limits simultaneous open files, buffers, metadata requests, and memory use.

## 3. Code Map

The workspace keeps a reusable library surface in `src/lib.rs`, a small CLI executable in `src/main.rs`, and a Tauri child package that depends on the root package by path.

- `src/main.rs`: parse process-level errors into exit codes and install signal handling.
- `src/cli.rs`: Clap arguments and conversion into validated `RunConfig`. Change user-facing flags here.
- `src/config.rs`: normalized input/output roots, `OutputLayout`, copy/overwrite policy, concurrency limits, key-source settings, and timeouts.
- `src/app/`: shared orchestration.
  - `events.rs`: UI-neutral batch lifecycle/report types and the opt-in `BatchObserver` seam. A disabled observer keeps CLI/watch paths free of desktop-only metadata/report allocation.
  - `scan.rs`: recursive initial enumeration, empty-directory mirroring, and output-subtree pruning.
  - `watch.rs`: recursive notifications, debouncing, file-stability checks, per-path deduplication, and requeue-on-change.
  - `runner.rs`: bounded scheduling, cancellation, summaries, and batch/watch error policy.
  - `job.rs`: the per-file state machine from classification through atomic publication.
- `src/pathing.rs`: all relative-path and destination calculations, including mirror/flat layouts and collision detection.
- `src/formats/`: encrypted-format registry and adapters. Change cipher dispatch here, not in the CLI.
  - `ncm.rs`, `qmc.rs`, `kgm.rs`, `kwm.rs`, `xiami.rs`, `ximalaya.rs`, and `tm.rs` implement Go-reference parity.
  - Additional adapters for the Rust library's JOOX, Migu 3D, and Qingting FM formats can use the same trait without changing orchestration.
  - `audio.rs` recognizes already-standard audio so encrypted-looking suffixes are not blindly decrypted or copied.
- `src/crypto/`: crate-private, selectively ported audio detection and NCM/QMC/KGM/Kuwo/Xiami/Ximalaya primitives. Application policy never enters this boundary.
- `src/keys/`: QMC key maps/MMKV discovery and KGM v5 database lookup. Key acquisition is separate from cipher adapters.
- `src/metadata/`: typed tags, provenance, merge rules, cover validation, tag read/write, and provider clients.
  - `provider/qq.rs` owns QQ Music RPC/search/cover requests.
  - NCM container metadata is parsed locally; its cover URL is the only NCM network request needed.
- `src/io/atomic.rs`: same-directory temporary outputs, flush/close, overwrite policy, and final rename.
- `src/error.rs`: typed library errors and user-facing context.
- `tests/fixtures/`: small legal/synthetic encrypted fixtures and expected outputs.
- `tests/`: CLI, directory-layout, watch, metadata, failure-isolation, and end-to-end tests.
- `benches/`: streaming and multi-file throughput benchmarks used to tune buffers and default concurrency.
- `desktop/`: Vite + React frontend and its `src-tauri/` Rust package.
  - `src-tauri/src/commands/`: narrow Tauri commands, DTOs, bounded persistence coordinator, ordered IPC `Channel` forwarding, ID-only reruns, and output-only metadata edits.
  - `src-tauri/src/history/`: Rust-owned SQLite migrations, run/item/artwork/preferences store, keyset run reads, SQL-backed song queries/facets, bounded output/display-metadata fact repair, persisted task names, pending metadata-edit recovery, deduplicated local artwork candidates, complete theme ID and safe player-volume preferences, online legacy-identifier backup, and record-only deletion.
  - `src-tauri/src/audio_protocol.rs`: item-ID-only content probing plus bounded `HEAD`/byte-range media responses for in-app playback; it never accepts a filesystem path.
  - `src-tauri/src/state.rs`: one-active-run ownership and cancellation state.
  - `src/`: Chinese/English/Japanese composer/history/detail UI, a complete semantic theme registry, and a route-independent persistent audio controller. The webview receives typed DTOs, raw artwork bytes, and item-ID media URLs only; it has no SQL, generic filesystem, process, shell, asset-protocol, or opener capability.

## 4. Main Execution Flow

1. `cli` parses `unlockm [OPTIONS] <INPUT>`. `RunConfig` canonicalizes paths, defaults output to the input directory (or a file's parent), and rejects invalid combinations.
2. `scan` emits file entries and, in mirror mode, empty directories. A proper output subtree inside the input is pruned to prevent recursive `output/output` processing.
3. `formats` classifies each source and decrypts at most a 128 KiB prefix to resolve the native extension. `pathing` then computes every destination and rejects all same-run claimants before any full stage is created. Mirror mode preserves the input-relative path; `--flat` uses only the basename.
4. `formats` first selects candidates by longest, case-insensitive suffix, then validates headers. Standard audio is passthrough; an invalid encrypted-looking file is an error unless standard-audio probing positively identifies it.
5. `runner` admits only conflict-free jobs to at most `--jobs` blocking workers. A decrypt adapter streams payload chunks through the crate-private local cipher primitives into a temporary file beside the final destination. Copies use `std::io::copy` into an atomic temp. A non-overwrite destination is skipped before this heavy phase and rechecked at publication.
6. The full decrypt trusts the already validated prefix extension and therefore never flushes/seeks/rereads the completed stage solely for sniffing. Per-job reports split probe, decrypt, metadata resolve, tag write, fallback, copy, and publish timing.
7. Without `--update-metadata`, decrypted audio bytes and any existing embedded tags are retained. With it, `metadata` resolves and validates all evidence before mutating the unpublished stage. FLAC metadata blocks are rewritten with bounded memory and 1 MiB audio-frame shifting; other containers use Lofty. A post-mutation failure discards the stage and regenerates clean audio only if the source fingerprint is unchanged and the job is not cancelled.
8. `atomic` publishes the completed temporary file. Existing outputs are skipped unless `--overwrite` is set. A failure leaves the source and any pre-existing output untouched.
9. Batch mode continues after per-file failures and returns a non-zero status after printing a summary. Watch mode logs per-file failures and stays alive until cancellation or a fatal watcher/configuration error.
10. The desktop command reserves a persistent `planning` run before scanning, then streams ordered lifecycle events through a bounded coordinator. Each terminal item, trusted local NCM/filename display snapshot, final output byte length, bitrate, and content-probed format commit to SQLite before the matching IPC event. A rerun reads its terminal run snapshot on the Rust side, revalidates it, and under the one-active-run lock transactionally clears that same terminal run's old items/counters/errors/orphaned artwork before returning the original ID to the identical conversion pipeline. It preserves the Rust-owned request and display name, so a rerun never adds a history row or exposes a half-reset run. A run can be cancelled but sources and incomplete private stages are never published. On startup, stale planning/running rows become `interrupted`; the webview keyset-pages runs, executes filtered/sorted song queries before page slicing, and lazily obtains artwork bytes. Schema-v1/older rows repair missing output facts and local display metadata only when their bounded detail page is read.
11. Metadata editing accepts only a terminal successful music item ID plus bounded tag values and image bytes (or a local artwork ID). Rust revalidates the output as a non-symlink regular file, writes and verifies a same-directory candidate, marks pending before publication, then refreshes SQLite from the real output; interrupted pending edits are repaired on detail read. Its webview dialog is viewport-bounded with a scrollable body and fixed actions. In-app playback uses one frontend audio controller outside page transitions, applies its persisted/clamped volume before the first `play()`, loops the current native audio element, clears/revokes artwork URLs on every track change, and addresses output through `unlockm-audio://` item identities. Rust maps the ID back to a successful persisted music output (including copied standard audio), rejects changed/symlink/unsupported targets, and returns at most a bounded byte range. Final-output container probing uses bounded checked seeks so leading ID3/APE artwork does not cause valid MP3 files to become unknown. Reveal and system-open commands follow the same ID-only lookup and validation boundary.

Watch mode performs the same initial scan, then converts normalized create, modify, and rename-to events through steps 3-8. Events are debounced and a file must have stable size and modification time across consecutive checks before it enters the runner. Rename-from/remove cancels queued work (and prevents a running job from publishing); a low-frequency reconciliation resubmits visible sources to cover notification loss without creating a second processing pipeline.

## 5. Architectural Boundaries

- `cli` depends on application/configuration types; it never calls a cipher, network provider, tag writer, or watcher directly.
- `app` coordinates traits and policies. It does not contain format-specific keys, suffix lists, or metadata-provider response models.
- `formats` may depend on crate-private `crypto` modules and `keys`; it does not decide destination paths or overwrite behavior.
- `crypto` contains format algorithms only. It does not know CLI flags, destination paths, metadata providers, job scheduling, or publication policy.
- `metadata` cannot decrypt files or publish outputs. Providers return typed candidates with identity evidence and never write tags themselves.
- `pathing` is pure apart from filesystem normalization. It does not probe file content.
- Only `io::atomic` may replace a destination. Adapters must write to the provided temporary sink and must not create final paths.
- `desktop/src-tauri` links `unlockm` directly and owns local history/IPC. It may construct validated desktop defaults but cannot duplicate format logic, the path planner, metadata rules, or publication.
- The desktop webview cannot access generic filesystem, database, shell, process, sidecar, asset-protocol, or opener capabilities. Its only core grants are event `listen`/`unlisten` for native drag/drop plus dialog open for choosing roots; Rust revalidates every selected path. ID-only custom commands may reveal/open previously persisted outputs, rerun an existing run, edit a validated music output with bounded fields/bytes, and query stored artwork candidates; the custom audio protocol may read bounded ranges from a validated persisted music item. None of those surfaces accepts a path from JavaScript.
- Online metadata is a separate, explicit desktop provider seam. MusicBrainz is the no-login primary text provider (plain-text cross-field dismax query, meaningful User-Agent, process-wide 1 rps gate, 10-second DNS/TCP/TLS connect bound, and 20-second total request bound). Artwork is explicit/on-demand: Cover Art Archive release-group images are primary and TheAudioDB `123` is fallback; CAA alone may follow at most three validated HTTPS storage redirects to `archive.org` or subdomains. Spotify OAuth, token exchange, fixed-host HTTP, loopback callbacks, credential persistence, and external navigation remain optional secondary Rust-only behavior. The webview receives provider-neutral text candidates and short-lived opaque cover handles, never tokens, callback values, raw provider payloads, or image URLs. Cover bytes are bounded/validated and may only enter the existing metadata draft; Save remains the sole atomic file publication path.
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
- Desktop history is local-only SQLite data: it stores conversion facts, paths, bounded metadata snapshots, actual output byte/type facts, a canonical complete `themeId`, other explicit preferences, and deduplicated artwork, not audio payloads. The compatibility decoder maps legacy light/dark plus accent-palette JSON to a stable theme ID; new writes never restore the legacy fields. Record deletion never touches source/output paths.
- The canonical application identifier is `world.fansionia.unlockm`. On first launch after the earlier misspelling, SQLite's online backup API copies the legacy history into the new app-data directory only when no new database exists; it keeps the old database recoverable and never merges over new data.
- Observer-only desktop display snapshots may read trusted local NCM container metadata regardless of the tag-update preference. This has no network or audio-write side effect. NCM nested artist arrays are supported; strict `Artist - Title` filenames are a final display fallback when both container/output tags omit those fields.
- Desktop event persistence is commit-before-IPC and one-active-run. A coordinator/database failure requests cancellation and releases active ownership; it cannot make an already published output unsafe.
- No conversion host may spawn the CLI, invoke a conversion process/shell, or package an `unlockm` sidecar. Tauri/webview implementation processes are outside this conversion-process boundary. The narrow opener adapter is allowed only after an explicit user gesture and ID-based Rust validation of an existing history output.
- Ambiguous metadata is never committed. Existing tags and covers win over guesses.
- Opening an editor does not make an online request. Online search, login, retry, preview, and external navigation require a user gesture; provider sessions and cover handles are invalidated on Disconnect/new search/expiry. Refresh tokens are OS-credential-store-only; access tokens and OAuth/PKCE material are process-memory-only.
- Watch events for a path are coalesced; if a file changes during processing, exactly one follow-up run is scheduled. Watch jobs share the batch runner's job semaphore, key indexes, metadata cache, and output reservations.

## 7. Cross-Cutting Concerns

### Concurrency and performance

`--jobs` defaults to `min(2, std::thread::available_parallelism())`, based on the real mixed APFS metadata/copy matrix; explicit `--jobs` still permits CPU/cache-heavy tuning. A Tokio semaphore bounds blocking file jobs, while a smaller independent semaphore bounds metadata HTTP requests. The decrypt buffer defaults to 1 MiB and is configurable; benchmarks may justify changing the default without changing the architecture.

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

Image magic/MIME, byte limit, and decodability are checked before embedding. Native Rust tag loading and writing identifies content from opened staged bytes, never from a private staging filename extension; it preserves audio frames and avoids an FFmpeg runtime dependency. The common FLAC path preserves unknown Vorbis fields and non-front pictures while streaming audio movement; Lofty remains the adapter for MP3/Ogg/MP4. If a container/tag combination cannot be safely updated, conversion succeeds with clean decrypted bytes and one source-oriented warning.

### Testing and release

Unit tests cover path planning, suffix precedence, metadata merging, output reservations, key lookup, event coalescing, and one-terminal-report contracts. Integration tests use temporary trees and deterministic local HTTP fixtures. Watch tests wait on observable outputs rather than fixed sleeps. Desktop tests cover SQLite recovery/migration/query pagination/commit ordering, one-active-run/cancellation, output facts, ID-only byte ranges and native actions, IPC DTOs, React state, theme ID migration, semantic-token completeness, contrast, i18n/accessibility, bounded paged rendering, playback ownership, and lazy artwork. Release checks include locked Vite+ frontend validation through `vp`, Rust checks, and no-bundle Tauri compilation on macOS/Windows. Native visual evidence and any unavailable platform/viewport remain explicit gaps in the active ExecPlan rather than being inferred from compilation. The tag channel has no signing-service or user-managed signing-credential cost and emits checksum-backed unsigned preview prereleases: macOS uses only an ad-hoc app signature, Windows artifacts are explicitly `NotSigned`, and neither channel contains a conversion sidecar. GitHub-hosted runner and artifact usage remains subject to the repository owner's plan and quota.

## 8. Extension Points

- Add an encrypted format by implementing `FormatAdapter`, registering suffixes in `formats::registry`, and providing positive/negative/header-truncation fixtures. Orchestration and CLI code should not change.
- Add a key source by implementing `KeySource`; adapters ask by stable identifiers and never know whether a key came from a footer, file, MMKV store, or SQLite database.
- Add a metadata provider by implementing `MetadataProvider` and returning evidence-bearing candidates. Merge thresholds remain centralized.
- Add an output layout by extending `OutputLayout` and pure planner tests before changing scanning or watch behavior.
- Add a desktop theme by registering the same stable ID in `desktop/src-tauri/src/history/mod.rs::ThemeId` and `desktop/src/lib/theme-presets.ts`, defining every required semantic role in `desktop/src/styles/themes.css`, adding localized metadata and any required source notice, then extending Rust serialization/migration and frontend theme contract/contrast tests. Components consume semantic variables only and never branch on a theme ID.
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

- `src/crypto/` owns the cipher primitives, footer/header parsing, and audio type detection selectively ported from pinned `lib_um_crypto_rust` revision `5dad838...`. `Cargo.toml` and `Cargo.lock` contain no dependency on that repository; provenance is retained in source headers and `THIRD_PARTY_NOTICES.md`.
- Tauri v2 owns the desktop window/runtime, standard native menu with custom About metadata, dialog bridge, and custom audio protocol. The official opener plugin is called only from ID-validating Rust commands and has no generic frontend permission. The child crate depends on the root `unlockm` library rather than any `unlockm` executable.
- The desktop `rusqlite` store owns local history. It is separate from KGM key extraction and is never exposed to JavaScript as a database connection or path.
- Vite+, React, i18next, Motion, TanStack Virtual, Phosphor Icons, and locally bundled Fontsource fonts belong only to the desktop UI; frontend setup/install/check/test/build/Tauri invocation use the `vp` entry point with the locked pnpm backend. `bundle.resources` ships `THIRD_PARTY_NOTICES.md` and the complete SIL OFL-1.1 text alongside Fontsource's Bricolage Grotesque and Manrope assets.
- Tokio owns async orchestration, cancellation, and HTTP/watch integration; it does not run blocking decrypt loops directly.
- Clap owns the public CLI grammar.
- `notify` (with a debouncer layer) owns platform filesystem notifications. Application code supplies stability and deduplication semantics.
- Reqwest owns bounded HTTPS access for metadata and covers.
- Serde owns NCM/QMC/provider payload parsing.
- Lofty owns native tag reads and non-FLAC tag/picture writes. `src/metadata/flac.rs` owns the bounded common FLAC write path. Both remain adapter boundaries because container support may evolve.
- Tracing owns structured diagnostics.
- Tempfile or an equivalent small wrapper owns unique same-directory temporary files.
- A Rust SQLite binding is confined to KGM v5 key extraction after crate-private `src/crypto/kgm::decrypt_db` decrypts the database bytes.

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
│   ├── crypto/
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
├── desktop/
│   ├── src/
│   ├── src-tauri/
│   └── pnpm-lock.yaml
├── .github/workflows/
│   ├── ci.yml
│   └── desktop-release.yml
└── benches/
```

## 12. Design Decisions

- **One reusable engine plus thin hosts.** The root package remains the CLI and library; `desktop/src-tauri` is a workspace child because the desktop app is a real second consumer. It links the library by path so CLI, batch, and desktop share scanning, path safety, format adapters, metadata, and publication invariants.
- **Own a selective, pinned crypto snapshot behind a crate-private boundary.** Only primitives exercised by `unlockm` are retained from revision `5dad838...`; deterministic vectors guard byte parity and notices retain MIT/Apache-2.0 provenance. This removes upstream availability from the build while making future upstream synchronization an explicit reviewed migration.
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
