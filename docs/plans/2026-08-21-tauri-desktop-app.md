# Tauri Desktop App for `unlockm`

## Status

In Progress

## Objective

Build a polished Tauri v2 desktop application for macOS and Windows that links the existing `unlockm` Rust library directly into the application, converts one selected file or recursively converts one selected directory, supports native file/folder picking and drag-and-drop, exposes a concise set of pre-run options, and persists conversion history across application restarts.

The main view must represent each user-initiated conversion as exactly one history row. Opening a directory run transitions to a full detail view that presents its music items as one flat, virtualized list regardless of source nesting. Each music item shows its terminal outcome, source/output information, captured title/artist/album/duration, and album artwork when available. The first release is bilingual (`zh-CN` and `en-US`), visually modern and youthful, animation-rich without sacrificing accessibility, and distributed by GitHub Actions for macOS and Windows.

The desktop application must not start the `unlockm` CLI or any other conversion sidecar. All decryption, path planning, metadata handling, bounded concurrency, and atomic publication remain in the in-process Rust core.

## Scope

- Add a Tauri v2 desktop package under `desktop/`, with a React + TypeScript + Vite frontend and a Rust Tauri backend package under `desktop/src-tauri/`.
- Use the user-provided `/Users/peilin/Downloads/UnlockM Icon.png` as the sole source artwork for UnlockM application icons. Preserve an unmodified copy in the repository; do not redraw or replace it with an SVG. Generate all Tauri `.icns`, `.ico`, and PNG derivatives only through `rtk vp -C desktop exec tauri icon ...`, documenting any reproducible upscale required by the CLI.
- Turn the root package into the reusable engine/CLI member of a small Cargo workspace while keeping the existing `unlockm` CLI behavior and release build intact.
- Add a stable, UI-independent batch event/reporting surface to the root Rust library so a desktop host can receive ordered batch and per-file lifecycle data without scraping logs.
- Support exactly one file or exactly one directory per user-initiated run. Directory input remains recursive and uses the existing mirror path planner; “flat detail view” refers only to presentation, not `--flat` output semantics.
- Support native input selection, native output-directory selection, and OS-native file/folder drag-and-drop.
- Offer these initial quick settings before conversion:
  - conservatively update metadata and covers (`update_metadata`, off by default),
  - copy ordinary companion files for directory input (`copy_passthrough`, on by default),
  - atomically replace existing destinations (`overwrite`, off by default),
  - choose an output directory; default to the input file parent or the input directory, matching the CLI.
- Keep mirror layout, automatic job limits, streaming buffer size, metadata concurrency, and automatic key discovery as fixed core defaults in the initial UI. Do not expose flat output or manual key material in v1.
- Allow one active conversion run at a time and provide cancellation as a minimum safety control for large directory runs.
- Persist runs, per-path outcomes, technical error/warning details, music metadata snapshots, and deduplicated artwork in a local SQLite database located in Tauri's application-data directory.
- Recover incomplete `planning`/`running` rows as `interrupted` at next startup without touching source or output files.
- Implement paginated history reads and a virtualized detail list so large directory histories do not require loading or rendering every item at once.
- Provide Chinese and English UI resources, runtime language switching, locale persistence, translated product/error-code copy, and sensible filename/path fallbacks.
- Implement whole-view home/detail transitions, shared row/header motion, live progress animation, drag-over feedback, and reduced-motion behavior.
- Add component, state/reducer, Rust unit/integration, persistence/restart, and cross-platform build validation.
- Add GitHub Actions checks and tagged release packaging for a universal macOS DMG and a Windows x86_64 NSIS `.exe` installer.
- Publish those tag artifacts automatically as clearly labeled unsigned preview prereleases: macOS uses Tauri ad-hoc signing only, Windows uses no Authenticode certificate, all artifacts include SHA-256 files, and documentation gives least-privilege first-open guidance.
- Update `ARCHITECTURE.md`, user documentation, license notices, and build instructions when implementation changes the repository boundary.

## Non-Goals

- Watch mode in the desktop application.
- Running the `unlockm` executable as a subprocess, Tauri sidecar, shell command, or external service.
- Literal single-OS-process operation; Tauri and the platform webview may own implementation processes. The requirement is that conversion has no extra CLI/worker process and that the Rust engine is linked into the app.
- Linux, mobile, App Store, or Microsoft Store distribution in the initial release.
- A raw portable macOS binary outside an `.app` bundle, or a raw portable Windows binary with no installer/runtime assumptions. The release contract is one downloadable installer artifact per platform.
- Paid/protected-code-signing, macOS notarization, Windows Authenticode reputation, and a claim that preview artifacts are trusted production releases. A future signed channel may be added separately without changing the unsigned-preview contract.
- Audio playback, waveform display, library management, playlists, search across history, history export/import, cloud sync, accounts, telemetry, auto-update, or background/tray operation.
- Multiple simultaneous batch runs, multi-selection of unrelated roots, or queuing several roots from one drag operation.
- Exposing `--watch`, `--flat`, `--jobs`, `--buffer-size`, `--metadata-jobs`, QMC ekeys/maps/MMKV passwords, or KGM database paths in the initial UI.
- Audio transcoding, deleting encrypted sources, changing collision policy, weakening atomic publication, or changing metadata confidence rules.
- Fetching metadata or artwork solely to decorate history. History captures the final/local output state; network access remains gated by `update_metadata`.
- Full theming or more than `zh-CN` and `en-US` in the first release. The token and resource structure must remain extensible.

## Current Context

- Repository state at planning time is clean at `c2dcb69` on `main`. All existing files under `docs/plans/` are `Done`; there is no competing active desktop/Tauri plan.
- `Cargo.toml` currently defines one Rust 2024 package, `unlockm`, with both `src/lib.rs` and `src/main.rs`. `rusqlite` with bundled SQLite is already used for KGM key lookup, but there is no application-history store.
- `src/main.rs` is a thin CLI host. It delegates to `app::run`, which uses `src/app/scan.rs` and `src/app/runner.rs`; this is a sound basis for a second in-process host.
- `src/config.rs::RunConfig` owns validated/canonical input and output paths plus batch/watch options, but its many-argument constructor and CLI presentation field are awkward for a desktop caller.
- `src/app/runner.rs` already bounds blocking jobs with a shared semaphore, supports cancellation, performs preflight collision grouping before publication, and records internal `JobReport` values. Its report collector is test-only and reports currently omit absolute source/destination paths, action kind, error/warning details, and metadata snapshots.
- `src/app/job.rs` performs streaming decrypt/copy and atomic publication. Best-effort metadata warnings are currently emitted through `tracing`, so the desktop cannot reliably associate those warnings with a history item without a structured result.
- `src/metadata/tags.rs::read` content-probes audio and already returns title, album, artist, duration, and embedded cover bytes. It can be reused after a converted or safely skipped destination is known; history must not reimplement tag parsing in TypeScript.
- `src/app/scan.rs` is recursive, rejects symlinks, prunes an output subtree inside the input, and uses the same planner as the CLI. These containment and source-retention semantics remain authoritative.
- `ARCHITECTURE.md` currently calls a GUI a non-goal and describes a one-package layout. The new app is the concrete second consumer that justifies a workspace child and requires an architecture update during implementation.
- `.github/workflows/ci.yml` currently runs formatting, Clippy, tests, watch tests, and a release build on Ubuntu, macOS, and Windows. It has no Node/frontend or Tauri package/release jobs.
- The repository currently has no frontend or Tauri configuration, app icon set, or desktop-specific capability file. The workspace scaffold added during T1.1 intentionally has no Tauri dependency/configuration yet.
- The verified product icon source is `/Users/peilin/Downloads/UnlockM Icon.png` (512×512 indexed PNG with alpha): a gold unlocked-lock/music-note/scissors composition. It must be copied without modification into the repository before derivative assets are generated.
- Tauri v2 official guidance supports Rust commands, ordered IPC channels for streamed data, raw IPC responses, native dialog permissions, and webview drag/drop events. Relevant references, verified on 2026-08-21:
  - <https://v2.tauri.app/develop/calling-rust/>
  - <https://v2.tauri.app/develop/calling-frontend/>
  - <https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/>
  - <https://v2.tauri.app/plugin/dialog/>
  - <https://v2.tauri.app/distribute/>
  - <https://v2.tauri.app/distribute/pipelines/github/>

## Design and Decisions

### D1. Repository and package boundary

Keep the root `unlockm` package as the engine plus CLI and add `desktop/src-tauri` as a second Cargo package depending on `unlockm = { path = "../.." }`. Add a root workspace with the desktop package as a member and the root package as the default member so existing `cargo build --release --locked` continues to build the CLI unless the desktop package is explicitly selected.

The desktop frontend lives beside the Tauri crate in `desktop/`. Planned structure:

```text
desktop/
├── package.json
├── <Vite+-generated lockfile>
├── vite.config.ts
├── index.html
├── src/
│   ├── app/
│   ├── components/
│   ├── features/conversion/
│   ├── features/history/
│   ├── i18n/locales/{zh-CN,en-US}.json
│   ├── lib/
│   ├── styles/
│   └── test/
└── src-tauri/
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    ├── capabilities/default.json
    ├── icons/
    └── src/
        ├── commands/
        ├── history/
        ├── lib.rs
        ├── main.rs
        └── state.rs
```

Use the local Vite+ frontend workflow exclusively: scaffold, dependency resolution, development, checking, testing, building, and Tauri CLI invocation must run as `rtk vp -C desktop ...`. Configure Vite+ to use the current stable pnpm backend internally; no raw `npm`, `pnpm`, `yarn`, or `bun` command is permitted in local instructions, automation, or CI. Commit the Vite+-generated `pnpm-lock.yaml` and pin compatible, current stable Tauri v2 Rust/JavaScript packages rather than floating versions in CI.

### D2. In-process conversion architecture

The desktop Tauri crate links the root `unlockm` library. It must not include the shell plugin, invoke `std::process::Command`, bundle the CLI as a resource/sidecar, or communicate with a local daemon. CPU/filesystem work continues through the existing Tokio plus bounded `spawn_blocking` design. The webview calls narrow Tauri commands; those commands construct validated core requests and call the library directly.

The existing CLI and desktop app use the same scanner, path planner, format registry, key sources, metadata pipeline, cancellation object, output reservations, and atomic publisher. Desktop-specific persistence and DTOs remain in `desktop/src-tauri` and must not leak Tauri types into the root library.

### D3. Stable batch events and per-file reports

Add `src/app/events.rs` with UI-neutral serializable domain types and an observer seam. The exact names may adjust during implementation, but the contract must express:

- batch planning started and total accepted paths,
- per-path planning/processing start with source, relative path, planned destination when known, and action kind,
- terminal per-path report with one outcome, logical bytes, timing fields, error code/message, zero or more non-fatal warnings, and optional music snapshot,
- terminal batch summary and cancellation/interruption distinction.

Use a synchronous `BatchEventSink: Send + Sync` (or equivalent callback interface) in the core. The desktop implementation forwards into a bounded channel consumed by one desktop coordinator, providing backpressure and preserving bounded memory and event order. A disabled/no-op sink keeps the CLI cheap. Never make a GUI/database failure change the safety or success semantics of already completed conversion work; coordinator failures are surfaced as application-level errors and cancellation, not silently ignored.

Every path admitted to a batch must produce exactly one terminal report, including preflight failure, same-run collision, existing-destination skip, copy, conversion, and cancellation after admission. Preserve `Summary` counters and CLI exit-code semantics.

Refactor metadata fallback paths to return structured non-fatal warnings as well as tracing them. Do not scrape `tracing` output. Add destination and error data at the job boundary where source context is known, preserving the existing rule of one source-oriented warning.

### D4. Desktop-safe configuration surface

Replace the public many-argument construction burden with a typed `RunOptions`/builder (exact name may vary) that contains defaults once. Both Clap conversion and desktop commands build the same validated `RunConfig`; desktop always sets `watch = false`, `layout = Mirror`, automatic job limits, 1 MiB streaming buffer, automatic key discovery, and no manually supplied secret values.

Keep CLI flags and defaults unchanged. Add regression tests that the legacy Clap path and desktop-default builder yield equivalent core settings for overlapping features.

### D5. Input and quick-setting semantics

The home composer accepts exactly one root:

- File picker: one regular file.
- Folder picker: one directory.
- Drag/drop: exactly one regular file or one directory; zero, multiple, mixed, symlink-only, or nonexistent paths are rejected with a localized validation message before a run row is created.

The Rust command revalidates and canonicalizes all frontend-provided paths; frontend validation is only UX. The output defaults exactly as the CLI does. Directory output remains mirrored. For a file, selecting an output means a directory, never an arbitrary final filename.

Quick settings are summarized inline and editable in a compact animated panel before the Convert button. `update_metadata` explicitly explains that verified network metadata may be requested. `copy_passthrough` is disabled/hidden for single-file input. `overwrite` warns that replacement remains atomic. Persist the user's locale and quick-setting defaults as app preferences, but snapshot the actual settings into every run record so history remains truthful if defaults later change.

Only one run may be active. Starting another returns a typed `busy` error. Cancellation uses the existing core cancellation token, never deletes sources, never publishes an incomplete private stage, and yields a terminal `cancelled` run status after workers stop.

### D6. Desktop command and event boundary

Expose a small command set from `desktop/src-tauri/src/commands/`:

- `bootstrap`: current preferences, active-run summary if any, and the first history page.
- `validate_input`: authoritative file/directory validation for picker/drop UX.
- `start_conversion(request, on_event: Channel<ConversionEventDto>)`: create the persistent run before work, mark it active, stream ordered progress, and resolve with the terminal run summary.
- `cancel_conversion(run_id)`: idempotently request cancellation only for the matching active run.
- `list_runs(cursor, limit)`: newest-first keyset pagination.
- `get_run_items(run_id, cursor, limit)`: stable ordinal pagination of flattened music items plus aggregate companion counts.
- `get_artwork(artwork_id)` plus `get_artwork_mime(artwork_id)`: return raw bytes and the validated MIME through two narrow Tauri IPC calls, because Tauri's raw `Response` body does not carry a custom MIME header; the frontend creates/revokes object URLs and lazy-loads visible covers.
- `delete_run(run_id)` and `clear_history`: remove history/artwork references only after explicit confirmation and never touch source/output files.
- `get_preferences` / `set_preferences`: locale and quick-setting defaults.

Use a Tauri IPC `Channel` for ordered run events instead of global event names. DTOs use camelCase at the IPC boundary while core and database types remain idiomatic Rust. Generate or manually maintain one TypeScript contract module and add serialization contract tests to prevent silent field drift.

### D7. Persistent history and crash recovery

Use a Rust-owned `rusqlite` store in the Tauri application-data directory. Do not expose a generic SQL plugin or database path to the webview. Initialize the directory and schema during Tauri setup before the main view leaves its bootstrap state. Use `PRAGMA user_version` migrations, foreign keys, WAL mode, bounded busy timeout, and explicit transactions.

Initial logical schema:

| Table | Purpose and required fields |
| --- | --- |
| `runs` | Integer id, input kind/path, output path, settings snapshot, status, timestamps, total/completed counts, all `Summary` counters, app/schema version, representative artwork id, terminal error code/detail. |
| `items` | Integer id, run id, stable ordinal, item kind, source/relative/destination paths, action, outcome, bytes, timings, error code/detail, warning JSON, title, artist, album, duration, artwork id. |
| `artwork` | Integer id, SHA-256 digest unique key, validated MIME, original bytes, byte length, created timestamp. Identical covers are stored once. |
| `preferences` | Key/value settings with schema-versioned JSON for locale and quick-setting defaults. |

Write run/item transitions through one blocking database coordinator. Commit terminal item state before forwarding the corresponding terminal UI event so a webview reload cannot display progress that is absent from history. Query commands run with separate short-lived/read connections or a properly serialized store and never block the Tauri main thread.

At startup, atomically convert stale `planning` or `running` runs to `interrupted`, set a recovery timestamp/message, and retain completed item records. Atomic core publication means an item may already have a valid output even if the process stopped before recording its terminal report; v1 does not guess or rescan to upgrade that record.

Deletion uses foreign keys and a post-delete orphan-artwork cleanup transaction. No operation recursively deletes files from user-selected paths.

### D8. Music metadata and artwork snapshots

Extend terminal reporting so successfully converted audio reads its final destination through `metadata::tags::read` after publication. A safely skipped audio destination may also be read if it exists and content-probes as supported audio. Failed/conflicted items use a filename fallback and placeholder artwork; they must remain visible with their error.

Do not show ordinary copied companions as music rows. Persist them for accurate counts/diagnostics with `item_kind = companion`, while the detail query returns music items by default and a compact companion summary. Encrypted candidates that fail before destination planning remain music items when their registered suffix identifies them as such.

Copy only normalized display fields and validated cover bytes into the history snapshot. Never store encrypted or decrypted audio payloads in SQLite. Deduplicate artwork by digest, enforce the core's existing 10 MiB cover ceiling, load bytes lazily, and revoke frontend object URLs when rows leave the viewport. Missing/corrupt artwork degrades to a deterministic local placeholder.

### D9. Product UI, visual direction, and motion

Use React function components, a typed reducer/context for conversion state, and feature-local hooks. Avoid a broad state-management framework in v1. Use `motion/react` for shared layout/view transitions and `@tanstack/react-virtual` (or a verified equivalent) for the detail list. Keep all color, typography, spacing, radius, elevation, duration, and easing values in CSS custom-property tokens.

Visual direction: a deep graphite/indigo base with restrained electric-violet and cyan accents, a warm coral highlight, high-contrast neutral text, crisp opaque surfaces, and album artwork as the richest color source. Avoid indiscriminate glass blur and gradient-heavy template styling. Use a compact native-app density, generous hit targets, the supplied gold UnlockM lock/music-mark, and visible keyboard focus.

Home view:

- top brand/status bar with language control,
- one large but compact drop/select composer,
- animated quick-settings strip and primary Convert action,
- one live/recent row per run, newest first, showing input name/type, status, counts/progress, timestamp, and a representative artwork/placeholder,
- empty, bootstrap, validation, busy, and history-error states designed explicitly.

Detail view:

- whole-view slide/fade transition from home with reverse back navigation,
- shared layout transition from the selected history row into a run summary header,
- flat virtualized music list with cover, title, artist, album, duration, outcome, and expandable path/error/warning details,
- skeleton and pagination states that do not shift the entire layout.

Motion guidelines: 180–320 ms for primary transitions, spring/layout motion only where it clarifies continuity, no perpetual decorative animation, and no animation that blocks conversion feedback. Under `prefers-reduced-motion`, remove travel/stagger/parallax and retain only near-instant opacity/state cues. Clean up all event listeners, channels, timers, and artwork URLs on unmount.

### D10. Internationalization and error presentation

Use `i18next` + `react-i18next` with complete `zh-CN` and `en-US` resources. Detect the platform/browser locale on first launch, fall back to English for unsupported locales, and persist explicit user selection. Keep translation keys semantic and prohibit hard-coded user-facing strings in feature components.

Rust commands return stable error codes plus interpolation parameters and a separate technical detail. The frontend translates the product message and reveals/copies the technical detail only in expanded diagnostics. Stored history retains both the code and original technical detail so later app versions can retranslate the product copy.

Test both locales for missing keys, interpolation, plural/count text, long English layout, Chinese punctuation, path truncation, and reduced-motion behavior. Dates/times and durations use `Intl` formatting, not hand-built locale strings.

### D11. Packaging and release interpretation

“Single executable” is interpreted as both:

1. the app contains the conversion engine and ships no `unlockm` sidecar or required FFmpeg/helper binary; and
2. each platform release offers one primary downloadable installer file.

Publish a universal Apple Silicon + Intel macOS DMG containing a Tauri ad-hoc-signed `.app`, and one unsigned Windows x86_64 NSIS setup `.exe`. Both are explicitly labeled unsigned previews and accompanied by SHA-256 files. The installed macOS application remains an `.app` bundle and Windows remains an installed application because those are platform conventions. Windows may bootstrap WebView2 when absent; embedding the roughly 127 MiB offline WebView2 installer is not the default unless offline installation becomes an explicit requirement.

Add a tag-driven `.github/workflows/desktop-release.yml` using the official `tauri-apps/tauri-action`. Add a non-release desktop build/check workflow or extend CI so both target OSes compile the Tauri crate and frontend before release. Release jobs must use locked Rust and Vite+-resolved frontend dependencies, upload only the intended primary artifacts plus checksums, and fail if a CLI sidecar/resource appears in the bundle.

The tag workflow must require no GitHub environment, identity, certificate, password, or signing secret. It must set `APPLE_SIGNING_IDENTITY='-'` for macOS ad-hoc signing, verify that specific signature mode without notarization/stapling, and assert that both the Windows app executable and NSIS installer are `NotSigned`. Release title/body and filenames must say `unsigned preview`; documentation must explain SHA-256 verification and the narrow platform-native first-open flow without asking users to disable Gatekeeper, SmartScreen, or other global protections.

### D12. Security and privacy boundary

Grant only the Tauri dialog permission and core window/event permissions actually used. Do not enable shell, generic filesystem, SQL, HTTP, updater, clipboard, or opener permissions by default. Rust owns all filesystem/database/network operations and canonicalizes paths again at the command boundary.

Set a restrictive CSP that permits packaged frontend assets, Tauri IPC, and local `blob:` URLs needed for artwork, but not arbitrary remote scripts/images. Album art is delivered from stored bytes, never rendered from a provider URL. Keep metadata networking inside the existing allowlisted Rust provider and only when `update_metadata` is true.

Never log or persist QMC/KGM secret material. History intentionally stores user-selected absolute paths locally; mention this in documentation and ensure no telemetry, analytics, crash upload, or cloud synchronization is introduced.

## Milestones

### M1. Reusable observable batch engine

- Goal: give both CLI and desktop callers one validated batch API with structured, bounded, per-file lifecycle reporting.
- Dependencies: existing scanner, runner, job, metadata, and cancellation modules.
- Acceptance criteria:
  - CLI output, exit codes, watch behavior, path/collision policy, source retention, and atomic publication remain unchanged.
  - Every admitted path produces one terminal report with source, optional destination, action/item kind, outcome, timings, errors/warnings, and optional music snapshot.
  - Core events contain no Tauri/frontend/database types.
  - Event delivery and report creation remain bounded for large batches.
- Validation:
  - `rtk cargo fmt --all -- --check`
  - `rtk cargo clippy --all-targets --all-features -- -D warnings`
  - `rtk cargo test --all-targets --all-features`
  - Existing serialized watch suites remain passing.

### M2. Tauri shell, secure IPC, and durable history

- Goal: create a bootable desktop package whose Rust backend validates requests, runs one in-process batch, streams events, and transactionally persists/reloads history.
- Dependencies: M1.
- Acceptance criteria:
  - No sidecar/shell invocation exists in code or built bundle.
  - Database migrations are idempotent; restart retains runs/items/artwork/preferences; stale active rows recover as `interrupted`.
  - One-active-run and idempotent cancellation semantics are enforced in Rust.
  - Commands are narrowly registered and capability/CSP files grant no unused broad access.
- Validation:
  - Rust history/command integration tests using temporary app-data roots.
  - `rtk cargo test -p unlockm-desktop --all-targets`
  - `rtk cargo clippy -p unlockm-desktop --all-targets -- -D warnings`
  - Manual database restart/recovery inspection with no source/output mutation.

### M3. Conversion composer and live run experience

- Goal: deliver the primary file/folder picker, drag/drop, quick settings, convert/cancel, and one-row-per-run live history interaction.
- Dependencies: M2.
- Acceptance criteria:
  - Picker and drag/drop validate one file or one directory and produce the same request semantics.
  - Settings are visible before conversion and snapshot accurately into history.
  - Live progress survives per-file failures, represents partial success accurately, and cannot launch a second run concurrently.
  - File and directory conversions use the core directly and preserve all output-safety invariants.
- Validation:
  - Frontend reducer/component tests with mocked Tauri commands and channel events.
  - Manual single-file, nested-directory, existing-output, metadata-off, metadata-on, and cancellation runs on macOS and Windows.
  - Verify no metadata network request when the setting is off.

### M4. History detail, artwork, i18n, and polished motion

- Goal: make persisted history and directory details feel like a coherent native product in both languages.
- Dependencies: M3.
- Acceptance criteria:
  - Home shows one row per run after restart; directory detail is a flat, paginated, virtualized music list.
  - Music rows show captured metadata and lazy artwork or a deterministic placeholder.
  - Home/detail transitions animate the entire view and selected-row continuity; reduced motion is honored.
  - Chinese and English cover all user-visible flows, statuses, confirmations, errors, and empty states.
  - Large histories do not load all artwork or DOM rows at once.
- Validation:
  - `rtk vp -C desktop check`
  - `rtk vp -C desktop test`
  - `rtk vp -C desktop build`
  - Visual QA at minimum 1024×700 and 1280×800 in both locales, with normal and reduced motion, on both platform webviews.

### M5. Documentation, cross-platform CI, and release artifacts

- Goal: make the desktop app reproducibly buildable and produce the intended primary artifacts on GitHub.
- Dependencies: M1–M4.
- Acceptance criteria:
  - `ARCHITECTURE.md` and READMEs describe the desktop boundary, data location/privacy, build commands, supported UI scope, and release artifact semantics.
  - CI validates Rust workspace and frontend on relevant hosts without regressing CLI checks.
  - Tag workflow produces one universal macOS DMG and one Windows x86_64 NSIS `.exe`, with checksums and no CLI sidecar.
  - Tag workflow produces a transparent unsigned preview prerelease with artifact/checksum/no-sidecar validation. Future paid signing/notarization is optional follow-up work, not a v1 completion gate.
- Validation:
  - `rtk cargo fmt --all -- --check`
  - `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `rtk cargo test --workspace --all-targets --all-features`
  - `rtk vp -C desktop install --frozen-lockfile`
  - `rtk vp -C desktop check`
  - Local/native `rtk vp -C desktop exec tauri build` smoke on macOS and Windows or equivalent successful Actions matrix evidence.
  - Inspect artifacts and launch/install on clean test users; verify restart history and conversion smoke.

## Task Breakdown

- [x] T1.1 Add the root Cargo workspace/member/default-member configuration and scaffold `desktop/src-tauri/Cargo.toml` so the desktop crate links `unlockm` by path; verify the existing default CLI build still selects only the root package.
- [x] T1.2 Refactor `src/config.rs` to provide typed defaults/builder construction shared by Clap and desktop requests; add equivalence and invalid-combination tests.
- [x] T1.3 Add `src/app/events.rs` domain event, action/item-kind, warning/error, metadata snapshot, and sink types with serialization/ordering unit tests.
- [x] T1.4 Extend `src/app::JobReport` and `src/app/runner.rs` so preflight failures, skips, conflicts, copies, conversions, and admitted cancellations each produce exactly one structured terminal report; verify counters remain unchanged.
- [x] T1.5 Refactor `src/app/job.rs` metadata and publication paths to return structured non-fatal warnings and terminal error details while retaining one source-oriented tracing message and atomic fallback behavior.
- [x] T1.6 Add post-publication/local-destination metadata snapshot extraction through `src/metadata/tags.rs`, including music/companion classification, cover ceiling enforcement, filename fallback, and no-network tests.
- [x] T1.7 Route `src/app::run` and watch initial batches through the new no-op/CLI sink without changing CLI/watch presentation; run all existing integration and watch regression suites.

- [x] T2.1 Scaffold `desktop/package.json`, Vite+-generated lockfile, Vite/React/TypeScript/Vitest/ESLint configuration, Tauri v2 configuration, and a minimal bilingual bootstrap view; copy the supplied original icon into the repository and generate only its Tauri derivatives with `rtk vp -C desktop exec tauri icon ...`; use only `rtk vp -C desktop ...` for scaffold/install/dev/check/test/build/Tauri invocations; verify deterministic frontend and Tauri dev builds.
- [x] T2.2 Implement `desktop/src-tauri/src/state.rs` with history store, active-run ownership, cancellation handle, and typed initialization failure behavior; unit-test one-active-run transitions.
- [x] T2.3 Implement versioned SQLite migration/bootstrap code under `desktop/src-tauri/src/history/` for `runs`, `items`, `artwork`, and `preferences`; test empty creation, reopen, migration idempotence, foreign keys, and WAL settings.
- [x] T2.4 Implement transactional run/item/artwork write APIs, SHA-256 artwork deduplication, pagination queries, orphan cleanup, and delete semantics that never access user output paths; test large-page boundaries and duplicate covers.
- [x] T2.5 Implement startup recovery that marks stale `planning`/`running` rows `interrupted` and preserves terminal items; add a simulated-crash/restart test.
- [x] T2.6 Implement Rust DTOs and Tauri commands for bootstrap, authoritative input validation, history pagination, artwork raw response, preferences, run deletion, and history clearing; add serde contract fixtures consumed by TypeScript tests.
- [x] T2.7 Implement `start_conversion` and `cancel_conversion` using the linked core, a bounded coordinator channel, Tauri IPC progress channel, transaction-before-terminal-event ordering, idempotent cancellation, and cleanup of active state on every exit path.
- [x] T2.8 Add minimal `capabilities/default.json` permissions and a restrictive CSP; add a repository check/test that rejects shell/sidecar/generic SQL/generic filesystem permissions and unexpected external asset origins.

- [x] T3.1 Build the frontend IPC adapter and typed app reducer under `desktop/src/lib/` and `desktop/src/app/`; test ordered event reduction, stale-run rejection, reconnect/bootstrap reconciliation, partial failure, and cancellation.
- [x] T3.2 Build the file/folder picker and drag/drop composer in `desktop/src/features/conversion/`, including drag states, exactly-one-root validation, keyboard access, and localized error handling; test picker/drop parity.
- [x] T3.3 Build the animated quick-settings panel and output-directory flow with correct defaults, directory-only companion toggle, metadata-network explanation, overwrite warning, and persisted preferences; test request snapshots.
- [x] T3.4 Build the Convert/cancel lifecycle and live main-history row with planning/running/terminal states, aggregate progress, partial success, busy protection, and safe retry affordance.
- [x] T3.5 Add end-to-end Rust integration fixtures that drive the desktop coordinator for one file and a nested directory, verifying output layout, terminal history rows/items, metadata-off networking, restart persistence, and cancellation safety.

- [x] T4.1 Define visual tokens, typography, responsive window layout, focus/hover/disabled/error states, and local placeholder artwork in `desktop/src/styles/` and `desktop/src/components/`; verify the Tauri `.icns`, `.ico`, and PNG icon derivatives were generated from the preserved supplied PNG (not redrawn or replaced by a source SVG) and that `tauri.conf.json` references them.
- [x] T4.2 Implement paginated newest-first main history with exactly one row per run, representative artwork, skeleton/empty/error states, stable selection, and explicit record-only delete/clear confirmations.
- [x] T4.3 Implement the full detail view with summary header, flat music-item pagination/virtualization, lazy artwork object URLs, metadata/duration display, status filters for diagnostics, and expandable source/output/error/warning details.
- [x] T4.4 Implement home/detail shared layout and whole-view transitions, live row/layout animation, reverse navigation, drag feedback, and comprehensive `prefers-reduced-motion` behavior; test listener/channel/object-URL cleanup.
- [x] T4.5 Add `i18next` setup and complete `zh-CN`/`en-US` resources, locale persistence/detection, Intl date/duration formatting, stable Rust error-code mapping, and missing-key/interpolation tests.
- [x] T4.6 Add component/interaction/visual-state tests covering both locales, long paths/text, thousands-item virtualization, missing/corrupt cover fallbacks, bootstrap/database errors, partial batches, and reduced motion.

- [x] T5.1 Update `ARCHITECTURE.md` for the workspace, desktop host, batch event seam, SQLite history boundary, IPC/security model, and the invariant that CLI/batch/watch reuse one core pipeline.
- [x] T5.2 Update `README.md`, `README.zh.md`, `.gitignore`, and third-party notices with desktop prerequisites, dev/build commands, scope differences from CLI, local-history/privacy behavior, and artifact semantics.
- [x] T5.3 Extend `.github/workflows/ci.yml` or add a focused desktop CI workflow for locked `vp -C desktop install --frozen-lockfile` checks and Tauri compilation on macOS/Windows while preserving all existing CLI/Linux/watch validation.
- [x] T5.4 Add `.github/workflows/desktop-release.yml` using pinned official actions, a universal macOS target and Windows x86_64 NSIS target, tag/version checks, checksums, unsigned-preview prerelease/artifact-name assertions, and no-sidecar validation.
- [x] T5.5 Configure unsigned preview release behavior with no signing-service or user-managed signing-credential cost: ad-hoc macOS signature verification, unsigned Windows app/NSIS verification, no secrets/environment, user-facing SHA-256/first-open guidance, GitHub quota disclosure, and future signing as an optional follow-up.
- [ ] T5.6 Perform final macOS and Windows install/launch/conversion/restart/history/manual-motion QA, inspect bundles for absence of `unlockm` sidecars and unexpected permissions, record exact commands/artifact hashes/results, and resolve review findings before marking the plan `Done`.

## Validation Strategy

### Core regression and contracts

- Preserve all current CLI fixture, layout, metadata, failure-isolation, logging, progress, and watch tests.
- Add table-driven tests ensuring one terminal report per admitted path and exact agreement between report outcomes and `Summary` counters.
- Test destination/error/warning presence for preflight failure, collision, skip, copy, conversion, metadata fallback, and cancellation.
- Test metadata snapshot extraction from extensionless staged/final audio by content, not filename extension, and prove it does not initiate provider traffic when metadata update is off.
- Test the typed config builder against existing CLI defaults and invalid combinations.

### Persistence and coordinator

- Use temporary app-data directories and real bundled SQLite for migration, reopen, crash recovery, pagination, artwork deduplication, delete/orphan cleanup, and preference tests.
- Assert database terminal state commits before the matching terminal IPC event is delivered.
- Simulate a frontend channel closing mid-run: conversion must remain safe, persistence must complete or the run must cancel with an explicit app-level error, and active state must always clear.
- Simulate process restart with an active row and completed items; only the run status changes to `interrupted`.
- Validate that history deletion changes only the app-data database and never stats/removes recorded source/output paths.

### Frontend

- Unit-test request construction and event reducers independently of React.
- Component-test picker/drop validation, quick setting visibility/defaults, busy/cancel states, one-row-per-run history, navigation, paginated detail rendering, diagnostics expansion, and deletion confirmation with mocked Tauri APIs.
- Run all tests once per locale and enforce a no-missing-translation-key check.
- Mock artwork raw responses and assert lazy load, object URL revocation, placeholder fallback, and bounded visible image requests under virtualization.
- Test keyboard navigation, visible focus, accessible names/live regions, contrast, and reduced-motion media queries.

### Manual functional matrix

- macOS Intel/Apple Silicon universal build: pick one file, drag one file, pick a nested folder, drag a folder, cancel mid-folder, run with each quick-setting combination, reopen app, open history detail, delete history without deleting output.
- Windows x86_64: repeat the same matrix, including a machine/user without a preexisting output directory and a system where WebView2 bootstrap behavior can be observed.
- Inputs: supported encrypted audio, invalid encrypted-looking file, standard audio, companion files, duplicate flat-looking basenames in separate directories, output inside input, existing destination with overwrite off/on, unicode/long paths, and missing/renamed output after history capture.
- Verify directory details are visually flat while output remains mirrored.
- Verify source files remain untouched and no half-written final destination is visible after cancellation/failure.
- Verify metadata-off makes no provider request; metadata-on provider/tag failures remain warnings with playable decrypted output.

### Build and release gates

- Local/common (the frontend lifecycle is intentionally Vite+-only):
  - `rtk cargo fmt --all -- --check`
  - `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `rtk cargo test --workspace --all-targets --all-features`
  - `rtk vp -C desktop install --frozen-lockfile`
  - `rtk vp -C desktop check`
  - `rtk vp -C desktop test`
  - `rtk vp -C desktop build`
- Native packaging:
  - macOS universal `rtk vp -C desktop exec tauri build --target universal-apple-darwin` producing a DMG, followed by architecture, bundle-content, explicit ad-hoc signature, launch, and checksum inspection; no notarization is expected for a preview.
  - Windows x86_64 `rtk vp -C desktop exec tauri build --target x86_64-pc-windows-msvc` producing one NSIS `.exe`, followed by explicit `NotSigned` app/installer state, install/uninstall, WebView2 behavior, launch, bundle-content, and checksum inspection.
- GitHub `v*` prerelease must attach exactly the two `unsigned` preview installers and their checksums; the title/body must explain platform warnings and SHA-256 verification.

## Risks and Mitigations

- **“Single executable” conflicts with platform packaging conventions.** A macOS GUI is normally distributed as an `.app` inside a DMG and Windows as an installer-managed app. This plan defines one downloadable artifact per platform and no conversion sidecar. If a raw portable binary was intended, reopen D11 before implementation because signing, icons, webview resources, and runtime behavior materially change.
- **Tauri itself may use platform webview processes.** The no-multiprocess product constraint is enforced at the application boundary: no spawned CLI/helper/daemon. Document this distinction and inspect bundles/source for sidecars and process launches.
- **Core reporting refactor could regress CLI/watch semantics.** Keep the event seam UI-neutral and disabled by default, preserve `Summary`, route both existing hosts through it, and require all current integration/watch suites before desktop work is accepted.
- **Large directories could overwhelm IPC, DOM, memory, or SQLite.** Use a bounded coordinator channel, one terminal item transaction at a time, keyset pagination, virtualized rows, lazy artwork, and no all-run report vector.
- **Artwork can bloat history.** Enforce the existing 10 MiB ceiling, deduplicate by SHA-256, never store audio payloads, lazy-load bytes, and remove orphan artwork transactionally when history is deleted.
- **Crash timing can make history lag a safely published output.** Commit history before terminal UI events, recover active runs as interrupted, retain completed items, and avoid guessing based on filesystem state in v1.
- **SQLite work can block UI or contend with conversion.** Own writes in one blocking coordinator, keep reads short/paginated, use WAL and a busy timeout, and never run database calls on the webview/main thread.
- **Drag/drop paths and Tauri permissions differ across macOS/Windows.** Use official webview drag/drop events and dialog plugin, revalidate in Rust, keep capabilities minimal, and execute the same manual matrix on both platforms.
- **Unsigned previews trigger platform warnings.** Release body and README explain that the artifacts are unsigned previews, publish SHA-256 values, and give only the minimal per-app first-open choices. Never ask users to disable Gatekeeper, SmartScreen, or other system-wide protections; signed/notarized distribution is a future optional channel.
- **Universal macOS and WebView2 choices affect artifact size.** Build/test both Apple architectures in one universal DMG. Use the normal Windows WebView2 bootstrap strategy initially; switch to the large offline bundle only through a recorded decision if offline install is required.
- **History stores privacy-sensitive absolute paths.** Keep storage local, expose clear/delete controls that never delete outputs, add no telemetry/cloud sync, and document the data location and semantics.
- **Animation can harm accessibility or performance.** Limit durations and animated properties, virtualize large lists, honor reduced motion, and include webview-specific visual/performance QA rather than relying only on browser mocks.

## Progress

- [x] 2026-08-21: Read `AGENTS.md`, parent RTK guidance, `docs/PLANS.md`, `ARCHITECTURE.md`, package/CI configuration, core batch/job/config/metadata paths, current completed plans, and official Tauri v2 command/dialog/drag-drop/distribution guidance.
- [x] 2026-08-21: Chose the in-process workspace child, structured event seam, Rust-owned SQLite history, React/Vite UI, bilingual motion design, and one-installer-per-platform release interpretation.
- [x] 2026-08-21: Created this `Ready` ExecPlan. No production source, test, build, or workflow files were changed in the Plan phase.
- [x] 2026-08-21: Began implementation after verifying local `vp v0.2.8`. The desktop package does not exist yet, so Vite+ correctly reports no local Vite/Vitest/Ox tools until T2.1 scaffolds and installs them.
- [x] 2026-08-21: Completed T1.1. Added the `desktop/src-tauri` workspace member linking `unlockm` by path, with the root package as the sole default member. `rtk cargo generate-lockfile --offline` refreshed the workspace lock; `rtk cargo build --locked` passed with the required sccache permission, and `rtk cargo metadata --no-deps --format-version 1` confirmed the default-member boundary.
- [x] 2026-08-21: Completed T1.2. Added `RunOptions` with one authoritative automatic/default policy and moved Clap conversion to it; kept `RunConfig::new` as a compatibility adapter. `rtk cargo fmt --all -- --check` and `rtk cargo test --locked --lib` passed (93 tests) with sccache access.
- [x] 2026-08-21: T1.3–T1.7 implementation and its initial validation completed, but independent review found unresolved correctness/performance gaps; the task checkboxes and M1 completion claim are deliberately reopened pending the fixes below.
- [x] 2026-08-21: Implemented the M1 review fixes pending re-review: publish admission now drains one `JoinSet` result before admitting more than `jobs` in-flight tasks, so completed report/artwork payloads cannot accumulate per directory item; stable Tokio task-ID contexts produce terminal reports for preflight and publication `JoinError`s; snapshot extraction is opt-in through `BatchObserver::with_music_snapshots`, leaving CLI/watch/no-op full batches free of post-publication tag/cover reads; and tag reading checks an embedded picture's borrowed length before cloning it.
- [x] 2026-08-21: Added review-driven regressions for an actual MP3 containing a 12 MiB embedded BMP, no-op metadata-off full-batch observer behavior, converted/skipped/conflicted terminal/Summary agreement, and cancellation after a worker has been admitted. The bounded snapshot reader retains title/artist/album/duration while omitting only oversized/bad artwork with a warning; the default observer avoids snapshot/fallback/report-copy work. Third independent review passed all M1 fixes: `rtk cargo fmt --all -- --check`, strict workspace Clippy, and 140 Rust tests across 16 suites; its six focused regression probes also passed.
- [x] M1 review-fix is complete and independently approved: in-flight publish/report memory is bounded, no-op CLI/watch keeps the cheap path, `JoinError` has a terminal report, and artwork extraction is bounded before copying.
- [x] 2026-08-21: Completed T2.1. Vite+ scaffolded and owns the `desktop/` React/TypeScript lifecycle; package versions are locked, with TypeScript 5.9.3 and Vitest 4.1.10 selected for the current stable compatible peers. Added pinned Tauri 2.11.5/CLI 2.11.4/dialog 2.7.2 configuration, narrow initial dialog capability/CSP, one in-process `bootstrap` command, and a bilingual i18next startup view. The supplied 512px `UnlockM Icon.png` is preserved at `desktop/src-tauri/icons/source/` and generated into PNG/ICNS/ICO derivatives through `rtk vp -C desktop exec tauri icon ...`; the Tauri config references those derivatives. Removed the unused Vite placeholder page/assets after replacement.
- [x] 2026-08-21: T2.1 validation passed: `rtk vp -C desktop install --frozen-lockfile`; `rtk vp -C desktop check`; `rtk vp -C desktop test` (1 test); `rtk vp -C desktop build`; and `rtk vp -C desktop exec tauri build --debug --no-bundle`, which built `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/unlockm-desktop` with the linked core library.
- [x] 2026-08-21: T2.2–T2.5 initial implementation completed: `AppState` one-active-run ownership/cancellation and Rust-owned SQLite history in app data with schema-versioned migrations, FK/WAL/busy timeout settings, transactional terminal-item persistence, keyset pagination, artwork SHA-256 deduplication/orphan cleanup, record-only deletion, preference storage, and simulated startup recovery. This completion statement is superseded for T2.2 pending M2 re-review: admission/failure cleanup and active-history mutation protections are being strengthened.
- [x] 2026-08-21: T2.6 implementation and its command/fixture validation completed. T2.7 implementation is reopened after independent review found a run-admission ordering failure and an unfinalized runtime-initialization failure path; do not treat the initial coordinator validation as completion until both are repaired and re-reviewed.
- [x] 2026-08-21: T2.8 initial policy implementation completed. This statement is superseded pending M2 re-review because `core:default` is broader than the product boundary; replace it with the minimum explicit permission set and strengthen recursive no-sidecar/no-broad-plugin checks before rechecking T2.8.
- [x] 2026-08-21: M2 review fixes independently approved. Run admission creates/reserves the planning row before scan, transitions to running only after a successful scan, and finalizes scan/runtime failures without leaving an active slot; active history mutations are atomically gated against begin-run. Artwork MIME reads no BLOB, capability grants only dialog open, and recursive policy checks reject process/sidecar/broad-plugin boundaries. The independent reviewer passed formatter, strict Clippy, 18 desktop tests, Vite+ check/test/build, and Tauri debug no-bundle compilation; the subsequent full workspace regression passed 158 tests across 17 suites.
- [x] 2026-08-21: T3 initial implementation and validation completed but M3 review found unresolved bootstrap/request ownership, reload reconciliation, composer test coverage, and in-flight cancellation gaps; the task checkboxes and ready-for-review claim are deliberately reopened pending the fixes below.
- [x] 2026-08-21: Implemented M3 review fixes pending re-review. Vite+ now uses pinned pnpm 11.22.0 internally and `pnpm-lock.yaml`; npm lock/dev-engine configuration was removed. The reducer preserves a request-owned active state across late bootstraps, while a reload-recovered active run polls persisted bootstrap state with unmount cleanup until terminal. Picker/output/keyboard/busy/error composer mocks, conversion/preference request snapshots, and late-bootstrap/reload reducer tests were added. Cancellation now waits for a real private `.unlockm-stage-*` output from a repeatable 64 MiB NCM before cancelling, then verifies source retention, temporary-stage cleanup, no final output, and `Cancelled` history.
- [x] 2026-08-21: M3 review-fix validation passed: `rtk vp -C desktop install --frozen-lockfile` (pnpm 11.22.0), `rtk vp -C desktop fmt`, `rtk vp -C desktop check`, `rtk vp -C desktop test` (12 tests), `rtk vp -C desktop build`; `rtk cargo test --locked -p unlockm-desktop --all-targets` (21 tests), strict desktop Clippy, and `rtk vp -C desktop exec tauri build --debug --no-bundle`. Independent re-review remains required before rechecking T3.1–T3.5.
- [x] 2026-08-21: M3 review fixes independently approved with no remaining findings. The reviewer confirmed pnpm frozen install, 21 desktop tests, 12 frontend tests, Vite+ check/build, and Tauri no-bundle compilation. Bootstrap/request ownership, reload reconciliation, bounded UI state, composer interaction coverage, and in-flight cancellation are complete.
- [x] 2026-08-21: Initial T4.1–T4.6 implementation added tokenized visual styles under `desktop/src/styles/`, self-hosted pinned Bricolage Grotesque and Manrope fonts, responsive/focus/error/placeholder states, and reused the preserved UnlockM icon derivatives. Main history merges keyset pages without moving selected run identity, lazy-loads representative covers, and confirms both record-only delete and clear operations. This initial completion is superseded by the M4 review findings below; T4 checkboxes remain deliberately open until independent re-review.
- [x] 2026-08-21: Integrated the persisted detail route with direction-aware `AnimatePresence` travel, shared `layoutId` row/header continuity, reduced-motion zero travel, `@tanstack/react-virtual`, lazy raw artwork blobs with late-resolution URL revocation, true persisted music totals, filters, diagnostics, retry, and companion counts. Locales persist through Rust preferences; bootstrap applies them; all product errors map stable codes to translated copy while technical text remains collapsed in diagnostics. Locale parity/interpolation, Intl formatting, long partial rows, structured bootstrap errors, picker/listener cleanup, missing artwork, retry, and a mocked 1,000-item virtual window (12 rendered rows) are covered.
- [x] 2026-08-21: Initial M4 validation passed: `rtk vp -C desktop fmt`, `rtk vp -C desktop check`, `rtk vp -C desktop test` (25 tests), `rtk vp -C desktop build`, and `rtk vp -C desktop exec tauri build --debug --no-bundle`. Independent review subsequently reopened M4; do not begin M5 until the review-fix pass is independently approved.
- [x] 2026-08-21: Visual QA completed in a real local Vite+ render: bundled Bricolage/Manrope typography, deep ink/indigo/cyan/gold hierarchy, and the supplied icon were visually coherent. At 1527 px, 931 px, and 727 px viewport widths, `scrollWidth <= innerWidth`; the responsive two-column/single-column transition introduced no horizontal overflow. The local dev server was stopped after inspection.
- [x] 2026-08-21: Implemented M4 review fixes, pending independent re-review. Home and detail initial/pagination loads reserve stable skeleton rows; visible-gated home artwork uses `IntersectionObserver` with an API-unavailable fallback; a 30-row test proves only intersected covers invoke artwork IPC. Detail maps every persisted stable error/warning code to bilingual product copy, retains raw source/output/error detail only in collapsed diagnostics, uses persisted `completedItems`, and retains a 1,000-item virtual-window bound. Page/shared-layout transitions now use an explicit tween with duration `0.01` under reduced motion; native drag callbacks, artwork object URLs, and bootstrap/reload lifetimes have cleanup regressions. Core visual colors, typography, radii, durations, and easing are centralized in `styles/tokens.css`; the audit leaves only deliberate geometric `50%` circles in component CSS.
- [x] 2026-08-21: M4 review-fix validation passed: `rtk vp -C desktop fmt`; `rtk vp -C desktop check` (26 files, no lint/type warnings); `rtk vp -C desktop test` (11 files, 31 tests); `rtk vp -C desktop build`; and `rtk vp -C desktop exec tauri build --debug --no-bundle`, which rebuilt `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/unlockm-desktop`. M4 remains awaiting independent re-review; M5 is still paused.
- [x] 2026-08-21: A follow-up M4 review reopened two bounded interaction details. The interim target-gated history-row keyboard fix prevented nested delete activation from navigating (later superseded by the semantic sibling-controls fix below). Main-history keyset pagination now reserves a stable `run-skeleton pagination-skeleton` while its promise is pending, then removes it after resolution. Added the remaining `state_error` product code to both locale resources. T4 remains awaiting re-review and M5 remains paused.
- [x] 2026-08-21: Follow-up M4 validation passed: `rtk vp -C desktop fmt`; `rtk vp -C desktop check` (26 files, no lint/type warnings); `rtk vp -C desktop test` (11 files, 33 tests); `rtk vp -C desktop build`; and `rtk vp -C desktop exec tauri build --debug --no-bundle`, which rebuilt `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/unlockm-desktop` without bundle artifacts.
- [x] 2026-08-21: Final M4 accessibility review replaced the interim target-gated clickable history article with a semantic noninteractive `motion.article` containing two sibling native controls: an explicit, localized detail button that owns cover/copy/count/date and a separate delete button. Neither is nested in the other; both retain visible focus styles. Keyboard regressions prove Enter on detail only navigates, while Tab then Enter on delete only confirms/deletes; English and Chinese accessible detail labels are asserted. T4 remains awaiting independent re-review and M5 remains paused.
- [x] 2026-08-21: Final accessibility-fix validation passed: `rtk vp -C desktop fmt`; `rtk vp -C desktop check` (26 files, no lint/type warnings); `rtk vp -C desktop test` (11 files, 35 tests); `rtk vp -C desktop build`; and `rtk vp -C desktop exec tauri build --debug --no-bundle`, which rebuilt `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/unlockm-desktop`; `rtk git diff --check` also passed.
- [x] 2026-08-21: M4 independently approved with no remaining findings. The reviewer verified `rtk vp -C desktop check`, 18 focused M4 regressions, 21 desktop/Rust tests, and BrowserOS visual QA. T4.1–T4.6 are now complete; M5 is formally in progress.
- [x] 2026-08-21: M5 review reopened T5.2–T5.5. Tagged packaging did not independently run the complete Rust quality gate, Windows payload inspection stopped at signature verification, and Fontsource notices were not actually shipped with the Tauri bundle. The original completion claims are superseded until release Rust gates, NSIS payload inspection, and full OFL resource distribution/auditing are implemented and re-reviewed.
- [x] 2026-08-21: Implemented M5 review fixes, pending re-review. Package jobs now run `cargo fmt --all -- --check`, strict locked workspace Clippy, and locked workspace all-target tests before any signing/build action. Windows release validation uses runner `7z` to list/unpack NSIS, requires exactly one `unlockm-desktop.exe`, rejects `unlockm.exe`, FFmpeg/ffprobe, sidecar, and helper executables, and requires both license resources; real Windows install inspection remains T5.6 evidence. Added the complete SIL OFL-1.1 artifact at `third_party/licenses/SIL-OFL-1.1.txt`, exact Fontsource/Bricolage/Manrope attribution in notices, Tauri resource mappings for the OFL and notices, a Rust `license_audit` contract test, explicit CI/release license audit steps, and bundle resource assertions.
- [x] 2026-08-21: M5 review-fix validation passed: `rtk cargo fmt --all -- --check`; strict locked desktop Clippy; `license_audit` (1 test); `security_policy` (2 tests); locked full workspace formatter/Clippy/tests (162 tests, 18 suites); `rtk vp -C desktop install --frozen-lockfile`, `fmt/check/test` (35 tests), build, and Tauri debug no-bundle. The license audit now proves both Fontsource package and lockfile versions are exactly `5.3.0`, entry imports match, notices name the fonts/OFL, and both resource mappings are exact. Workflow YAML parse, Vite+-only command guard, and diff guard pass. A fresh debug DMG read-only mount physically contained `UnlockM.app/Contents/Resources/licenses/SIL-OFL-1.1.txt` and `THIRD_PARTY_NOTICES.md` alongside the app binary/icon, then was detached and its temporary mount directory removed. T5.2–T5.5 remain awaiting independent re-review; T5.6 remains open.
- [x] 2026-08-21: M5 independently approved with notes and no remaining implementation findings. The reviewer confirmed the final locked 162-test/18-suite local workspace validation, native macOS app/DMG QA, complete pre-signing release Rust gates, NSIS payload inspection, exact Fontsource OFL resource audit, and Vite+-only workflow boundary. T5.2–T5.5 are complete. T5.6 remains intentionally open, so the overall ExecPlan status remains In Progress.
- [x] 2026-08-22: Dependency audit evaluated newer React i18n/TypeScript/Vitest/Vite+ releases only through Vite+. Per user direction, the attempted tail-version upgrade was fully reverted in favor of the last complete validated frontend set: `react-i18next` 16.5.8, TypeScript 5.9.3, Vitest 4.1.10, and Vite+/Vite Core alias/override 0.2.8. Test imports, project script, docs, CI, release workflow, lockfile, and Vite+ lint policy all again use the native `vp test` path. `@types/node` remains lock-resolved at 24.13.3 because project and CI Node are intentionally 24, not the newer Node 26 type line. Restored-set validation passed: frozen install, `fmt/check/test` (35 tests), production build, Tauri debug no-bundle, and diff guard. T5.6 remains open.
- [x] 2026-08-22: Reopened T5.3 after Windows desktop CI reported formatting differences for all 42 frontend files while the same commit passed on macOS. `git ls-files --eol` confirms repository blobs/current macOS worktree are LF and no `.gitattributes` exists, so this is a Windows checkout CRLF normalization failure rather than a real multi-file formatter regression. The correction will force checked-out text to LF and use explicit install-outcome continuation conditions so an honest source-check failure does not hide platform compile results.
- [x] 2026-08-22: Recompleted T5.3 after the Windows CI fix. Root `.gitattributes` now enforces `text=auto eol=lf` while explicitly excluding common bundle/image/certificate binaries; `git check-attr`/`git ls-files --eol` confirm text source/configuration is LF and PNG is non-text. The existing Windows CRLF reproduction in an `autocrlf=true` temporary clone matched the 42-file CI failure; the attribute policy eliminates that checkout variance without weakening format enforcement. Desktop CI now records a named successful install, runs formatting as `vp -C desktop fmt . --check` and lint/type as `vp -C desktop check --no-fmt`, and uses `!cancelled() && steps.desktop_dependencies.outcome == 'success'` for test/frontend-build/Rust/Tauri steps so compile diagnostics still run after a quality-gate failure. Validation passed: frozen install, 42-file format check, lint/type check, 35 frontend tests, build, strict desktop Clippy, 22 desktop Rust tests across 4 suites, Tauri debug no-bundle, YAML parse, and diff guard.
- [x] 2026-08-22: User selected a distribution policy with no signing-service or user-managed signing-credential cost. Reopened T5.4–T5.5 before implementation: replace the protected credential/signing release flow with automatic `v*` unsigned preview prereleases, macOS ad-hoc signing, unsigned Windows verification, explicit SHA-256 assets, and least-privilege first-open documentation. This deliberately removes signing/notarization as a T5.6 completion condition while preserving T5.6 for real Actions and Windows install/launch evidence.
- [x] 2026-08-22: Recompleted T5.4–T5.5 for the unsigned preview channel. `desktop-release.yml` has no GitHub environment/secrets/certificates/notarization step; every `v*` tag runs the existing locked Rust/frontend/license/sidecar gates, builds universal macOS with `APPLE_SIGNING_IDENTITY='-'`, verifies `Signature=adhoc` without stapling, asserts Windows app/NSIS `NotSigned`, and emits exact `unsigned` asset/checksum names. Publish uses `--verify-tag`, `--prerelease`, warning-forward `--notes` plus generated notes, and asserts exact assets, prerelease state, and title `Unsigned Preview`. README English/Chinese document hashes and narrow Finder/SmartScreen per-app choices without advising global protection changes, and disclose that hosted-runner minutes/artifact storage remain subject to GitHub plan quotas. Validation passed: Rust fmt/strict workspace Clippy, license/security tests, frozen Vite+ install, 42-file format/lint/type, 35 frontend tests, build, Tauri no-bundle, YAML/static no-secret policy, diff guard, and a local universal app whose codesign details report `Signature=adhoc`, dual `x86_64 arm64`, expected license resources, and no sidecar. T5.6 remains open only for real Actions preview/Windows install evidence.
- [x] 2026-08-22: Optional local `actionlint` validation was unavailable (`No such file or directory`). Both workflows still parse as YAML and passed the focused action-pin, no-secret, Vite+-only, release-contract, and independent review checks; a real tag-triggered Actions run remains explicitly open under T5.6 rather than being inferred locally.
- [x] 2026-08-22: Independent unsigned-preview re-review passed with no remaining findings. The initial minor wording finding was resolved by limiting the cost claim to signing services/user-managed credentials and disclosing GitHub-hosted runner/artifact quotas in architecture, both READMEs, and D26. The reviewer confirmed the ad-hoc/`NotSigned` assertions, no-secret boundary, locked gates, prerelease command, exact title/assets, and accurate T5.6 gap.
- [x] 2026-08-22: Independent T5.3 re-review passed with no findings. The reviewer confirmed that the LF policy excludes binary bundle assets, real formatter/lint/test failures still fail the job, later compile diagnostics remain visible after successful dependency installation, and no `continue-on-error`, raw package-manager command, sidecar boundary, or release-signing relaxation was introduced.
- [x] 2026-08-21: Completed the initial T5.1–T5.5 implementation. The protected signing/secret documentation in this historical entry is superseded by the 2026-08-22 unsigned-preview policy; current READMEs explicitly say no release secrets are required and distinguish any future signed channel.
- [x] 2026-08-21: Reworked CI with SHA-pinned `actions/checkout` v7.0.1 and `voidzero-dev/setup-vp` v1.17.0. macOS/Windows desktop jobs use only `vp -C desktop` for locked pnpm-backed install/check/test/build/Tauri no-bundle work plus strict desktop Clippy, while retaining the existing locked three-platform CLI/watch matrix. The initial protected signed-release details in this historical entry are superseded by the 2026-08-22 unsigned-preview decision. Only the final publish job receives `contents: write`.
- [x] 2026-08-21: T5 static/local validation passed: both workflow YAML files parse; the no-raw-package-manager lifecycle guard passes; action references are immutable SHAs; `rtk cargo fmt --all -- --check`; strict `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`; `rtk cargo test --workspace --all-targets --all-features` (161 tests, 17 suites); `rtk vp -C desktop install --frozen-lockfile` (pnpm 11.22.0); `rtk vp -C desktop fmt/check/test` (35 tests); `rtk vp -C desktop build`; `rtk vp -C desktop exec tauri build --debug --no-bundle`; and `rtk git diff --check`.
- [x] 2026-08-21: Follow-up release-workflow precheck passed after the action/signing hardening: YAML parse, no raw package-manager lifecycle command guard, `rtk git diff --check`, and `rtk vp -C desktop check/test` (35 tests). The workflow uses macOS-compatible `base64 -D`, `lipo` to require both x86_64 and arm64 inside the signed app, PowerShell `Get-FileHash` for Windows checksums, and a temporary ignored Tauri Windows signing overlay so the primary app executable and NSIS installer are both signature-verified.
- [x] 2026-08-21: Native macOS app-bundle QA found that the Composer's official `onDragDropEvent` subscription needs Tauri core-event listener permission; the previous dialog-only capability showed the runtime banner `event.listen not allowed`. The capability is now exactly `core:event:allow-listen`, `core:event:allow-unlisten`, and `dialog:allow-open`. The security policy test asserts that exact allowlist and rejects `core:event:default`, event emit, path/menu/tray/webview defaults, and dialog default. This restores native drag/drop without widening filesystem/process/database access; T5.6 remains pending the resumed native QA and external release evidence.
- [x] 2026-08-21: Capability-fix validation passed: `rtk cargo fmt --all -- --check`; strict `rtk cargo clippy --locked -p unlockm-desktop --all-targets -- -D warnings`; `rtk cargo test --locked -p unlockm-desktop --test security_policy` (2 tests); `rtk cargo test --locked -p unlockm-desktop --all-targets` (21 tests, 3 suites); `rtk vp -C desktop fmt/check/test` (35 tests); `rtk vp -C desktop build`; and `rtk vp -C desktop exec tauri build --debug --no-bundle`. A local unsigned native DMG bundle was also produced at `target/release/bundle/dmg/UnlockM_0.1.0_aarch64.dmg` for resumed QA; it is not release evidence.
- [x] 2026-08-21: Completed the local macOS portion of T5.6 with a rebuilt debug `UnlockM.app` through Computer Use. The app launched without a Diagnostics/error banner, confirming the minimum `listen`/`unlisten` plus dialog capability. With metadata/overwrite off, `/Users/peilin/Codebase/um-rewrite/samples/Claire Rosinkranz - Frankenstein.ncm` converted to `/private/tmp/unlockm-native-qa.hSGQtj/Claire Rosinkranz - Frankenstein.flac`; `file` reported 16-bit stereo 44.1 kHz, 8,919,168 samples, and 23,548,419 bytes. Detail showed the fallback title, `3:22`, `converted`, and correct source/output diagnostics. A second conversion safely `Skipped` the existing output; the Chinese detail showed `已跳过`, with `1/1` completed. The NCM SHA-256 was unchanged before/after (`098f5f4c8a869b27cfd8c50efe54a914aa3652664848bc8f0b6d47feb8020b25`). After close/relaunch, both history rows and Chinese locale persisted; the app was closed. BrowserOS visual QA remains recorded above (three responsive widths, local bundled fonts/icon, no horizontal overflow).
- [x] 2026-08-21: Completed local DMG smoke evidence for the unsigned arm64 artifact `target/release/bundle/dmg/UnlockM_0.1.0_aarch64.dmg`: 9,098,799 bytes, SHA-256 `d1a52da9e7fee412ae48f056f0b3e93468a7e496a1cdb39a82e0367c449f1a71`, and `hdiutil verify` checksum `VALID`. A read-only mount contained only `UnlockM.app`, the `Applications` symlink, `.DS_Store`, and `.VolumeIcon`; the app bundle had only `Info.plist`, `Resources/icon.icns`, and `MacOS/unlockm-desktop`. The `MacOS` directory contained exactly one executable, which `file` identified as Mach-O arm64; no `unlockm` CLI/sidecar was present. The image was detached and its temporary mount directory cleared after inspection. This is a local unsigned arm64 smoke only, not a protected universal signed/notarized release claim.
- [x] 2026-08-22: Completed local universal DMG packaging evidence after installing the x86_64 Apple target: `target/universal-apple-darwin/release/bundle/dmg/UnlockM_0.1.0_universal.dmg` is 15,574,710 bytes with SHA-256 `15e891cde44724bc6cd76ca47ad40c8d03c31a6a45384ed1a0d79ef29f21dd6a`; `hdiutil verify` returned `VALID`. A read-only mount contained only `UnlockM.app`, `Applications`, `.DS_Store`, and `.VolumeIcon`. The app binary’s `lipo -archs` output was `x86_64 arm64`; the bundle contained only `Info.plist`, icon, SIL OFL, third-party notice, and the single `unlockm-desktop` executable, with no sidecar. `codesign --verify` reported `not signed at all`; the image was detached and mount directory cleared. This proves unsigned universal local packaging only—not protected signing/notarization—so T5.6 remains open.
- [ ] 2026-08-22: T5.6 remains pending the external no-secret preview matrix only: a real Actions `v*` unsigned prerelease, published artifact SHA-256 values, and a Windows x86_64 NSIS install/launch/conversion/restart inspection. Signing/notarization credentials are no longer a completion condition. M5 remains In Progress/reviewable until that Actions/Windows evidence is recorded.

## Discoveries

- The current library boundary is already strong enough to avoid a CLI subprocess, but the existing test-only report collector is not a product event API and cannot persist trustworthy per-item failures/warnings/artwork.
- The content-probing tag reader already supplies the exact local metadata/artwork snapshot needed by history; adding a second TypeScript parser or remote artwork layer would duplicate logic and weaken privacy/correctness.
- A directory detail can be visually flattened without enabling the core flat-output mode. Keeping mirror output avoids introducing same-name collision UX into the first desktop release.
- The existing `Cancellation`, shared job semaphore, path planner, and atomic publication make a safe desktop cancel control feasible without a second execution pipeline.
- Tauri v2 channels are a better fit than global events for ordered per-run progress, while raw IPC responses avoid JSON/base64 expansion for lazy artwork bytes.
- Standard Tauri distribution is platform-specific installers/app bundles and commonly requires signing. A literal bare executable is not the same product contract as a single downloadable installer artifact.
- Windows WebView2 can be bootstrapped in a small installer or embedded for a much larger offline installer; the initial plan favors the normal bootstrap because the user's no-sidecar constraint concerns conversion architecture, not the OS webview runtime.
- The user added a hard frontend-tooling constraint during implementation: `vp` is the only permitted frontend lifecycle command. Its current local help confirms create/install/check/test/build/run support, while tooling is absent until the frontend is bootstrapped.
- The workspace member changes `Cargo.lock` even before it introduces external dependencies, so an offline lockfile refresh is required before a locked root build can validate T1.1.
- Vite+ scaffolded the requested `vite --template react-ts` app through `vp`; because `desktop/src-tauri` already exists, it was generated in a temporary sibling subdirectory and its Vite+-generated project files were moved into `desktop/` without altering the Rust crate. The one generated temporary README/directory was removed after the transfer.
- **2026-08-21 frontend compatibility discovery:** `react-i18next` 16.5.8 and TypeScript 5.9.3 are the last verified stable pair for this Vite+ project. Vite+ 0.2.8’s native `vp test` runner uses Vitest 4.1.10; test imports intentionally use the matching `vite-plus/test` shim.
- **2026-08-22 stable-version decision:** A Vite+-only trial of the newer tail releases found that Vite+’s native test shim remains 4.1.10 while direct Vitest 4.1.11 requires a non-native adapter/import change. The user chose the complete, stable previously verified set instead of chasing that patch transition. All attempted dependency, lockfile, import, script, documentation, CI, and lint-policy changes were restored. `@types/node` remains lock-resolved at 24.13.3 to match the explicit Node 24 desktop/CI runtime rather than following Node 26 types.
- **2026-08-22 Windows CI formatting discovery:** Windows Git checkout can materialize CRLF without an explicit repository EOL policy, while Vite+ formatting expects LF. A whole-frontend formatting report immediately after checkout is diagnostic of checkout normalization, not evidence that each source file independently regressed.
- **2026-08-22 CI visibility discovery:** GitHub Actions skips ordinary later steps after a failure in the same job. An explicit `!cancelled()` condition gated only on the successful locked install preserves source-gate failure while still emitting native frontend/Rust/Tauri compilation evidence; no `continue-on-error` is used for the actual quality gates.
- **2026-08-22 preview-release discovery:** Tauri accepts `APPLE_SIGNING_IDENTITY='-'` for an ad-hoc macOS app signature. That gives a locally verifiable app-bundle signature without identity/certificate secrets, but it is neither notarization nor a trusted Developer ID signature. Windows preview artifacts must be asserted `NotSigned`, not represented as Authenticode-signed.
- **2026-08-22 unsigned-preview validation (resolved):** A PTY-managed local universal build completes with `Signing with identity "-"`, warns that notarization is skipped because no Apple credentials exist, and yields an app that passes `codesign --verify` with `Signature=adhoc`. This validates the intended preview contract without inventing production trust.
- **2026-08-22 cost-scope review finding (resolved):** No certificate, signing service, or user-managed signing credential is required, but GitHub-hosted macOS/Windows Actions minutes and artifact storage still consume the repository owner's plan quota and private-repository overages may be billable. README and architecture wording now state that boundary instead of claiming the whole pipeline is unconditionally free.
- **2026-08-21 review findings (M1, resolved):** (1) publish scheduling now drains while admitting and limits report/artwork payloads to active jobs; (2) snapshot capture is observer-capability-gated; (3) Tokio task-ID contexts produce per-source terminal `JoinError` reports; (4) regression coverage includes collision, skip, conversion, and cancellation/Summary agreement; and (5) picture length is checked before cloning.
- **2026-08-21 re-review findings (M1, resolved):** `read_bounded_snapshot` preserves text/duration while omitting oversized/bad artwork with a warning, proven with a titled 12 MiB embedded-cover fixture. Default observers are disabled; filename fallback and terminal report ownership are deferred to an enabled observer/test collector. Third independent review passed the core implementation and focused regression matrix.
- **2026-08-21 review findings (M2, resolved):** Admission now reserves/persists a zero-total planning row before scan; only a successful scan sets the total/running state, while scan/runtime failures finalize failed and release active ownership. Deterministic barriers prove delete/clear and begin-run cannot interleave.
- **2026-08-21 additional review findings (M2, resolved):** Capability now grants only dialog open; policy checks recursively reject broad core/process/sidecar/plugin configuration. `artwork_mime` selects only `id,mime_type`; delete/clear reject an active run. Third independent M2 review passed with no remaining findings.
- **2026-08-21 review findings (M3, unresolved):** (1) a late `booted` action must not replace local `starting`/request-owned active state with `bootstrap.activeRun`, or request IDs and terminal cleanup are lost; (2) a reload-recovered active run has no surviving IPC channel, so frontend state must periodically reconcile bootstrap/history and clear/refresh once the persisted terminal row is visible; (3) composer tests must mock picker/drop/output interactions, busy/keyboard/error behavior, and preference request snapshots; (4) cancellation needs deterministic proof after processing has genuinely started; and (5) switch the Vite+ package-manager backend and lockfile from npm to pnpm without raw pnpm invocations.
- **2026-08-21 review findings (M3, resolved):** Late bootstraps preserve local request ownership, reload-recovered active runs reconcile via cleaned-up polling, composer/settings tests exercise real request snapshots and mocked native interactions, and cancellation waits for a private staged 64 MiB NCM before cancelling. Vite+ uses pnpm internally through the required `vp` commands. Independent M3 re-review passed with no findings.
- **2026-08-21 review findings (M4, resolved):** Initial review required stable skeleton heights for bootstrap/detail pagination, bilingual item error-code product copy with raw diagnostics collapsed, `completedItems` counters, fully tokenized core visual primitives, `0.01` reduced-motion tween transitions without layout springs, and actual `IntersectionObserver`-gated home artwork. The implementation and focused regressions passed independent review.
- **2026-08-21 follow-up review findings (M4, resolved):** History-row keyboard target-gating was first used to prevent nested-control navigation, then superseded by semantic sibling native detail/delete controls in a noninteractive motion article. Newest-first history pagination renders a stable loading-row placeholder. The reviewer accepted the regression suite with no remaining findings.
- **2026-08-21 final accessibility finding (M4, resolved):** The sibling native detail/delete controls include localized accessible labels, focus treatment, and Tab/Enter behavior regressions. Independent M4 review passed with no remaining findings.
- **2026-08-21 channel lifecycle note:** Tauri `Channel` is owned by the outstanding `start_conversion` invocation and has no independent frontend close API. A webview reload therefore cannot retain that subscription; the app intentionally discards stale callbacks and reconciles a recovered active run from persisted `bootstrap` state with an unmounted timer cleanup.
- **2026-08-21 release-workflow discovery:** Vite+ CI uses the official `voidzero-dev/setup-vp` action; pinned v1.17.0 installs the project-selected `vp`, Node 24, and pnpm cache without introducing a package-manager action. The Tauri action must use `projectPath: desktop` for this nested Tauri package; `tauriScript: vp exec tauri` keeps its build invocation behind Vite+.
- **2026-08-21 release-workflow discovery:** macOS runner Bash lacks GNU-only `find -maxdepth`/`mapfile`, and Windows should compute SHA-256 with PowerShell `Get-FileHash`. The workflow uses portable per-platform commands, then verifies all checksums on Ubuntu before publishing an exact four-asset release set.
- **2026-08-21 release evidence gap (superseded by D26):** The initial protected-signing release approach required external credentials. The unsigned-preview policy replaces that scope; the remaining gap is a real Actions preview prerelease and Windows install/launch evidence, not paid signing/notarization.
- **2026-08-21 native bundle discovery:** Tauri's `getCurrentWindow().onDragDropEvent` is backed by the core event listener API, not only the dialog plugin. A dialog-only capability lets the app compile but produces an `event.listen not allowed` banner at runtime. The smallest correct grant is listener plus unlistener—never `core:event:default` or event emit.
- **2026-08-21 M5 review findings (resolved):** Tagged package jobs repeat the full locked Rust quality gate before signing, Windows NSIS payload inspection uses `7z` to check the real archived contents, and the complete OFL plus third-party notice are shipped through Tauri resources and checked in CI/release. Independent M5 review passed with no remaining implementation findings; T5.6 remains the separate real-release/Windows-install evidence gap.

## Decision Log

- **2026-08-21 — D1:** Add a separate Tauri workspace package depending on the root library instead of adding Tauri dependencies to the CLI package. This is the first real second consumer anticipated by `ARCHITECTURE.md`.
- **2026-08-21 — D2:** Treat “no multiprocess” as no spawned `unlockm` CLI/helper/daemon. Do not promise control over internal OS webview process implementation.
- **2026-08-21 — D3:** Add a structured core event/report sink and bounded desktop coordinator; do not parse logs and do not collect an unbounded batch vector.
- **2026-08-21 — D4:** Keep output mirror layout and core performance/key defaults fixed in the initial UI; expose only metadata update, companion copy, overwrite, and output directory.
- **2026-08-21 — D5:** Permit one file or one directory per run and one active run globally. Add cancellation as essential large-batch safety.
- **2026-08-21 — D6:** Persist history in Rust-owned bundled SQLite in app data, including snapshot metadata and deduplicated cover bytes; do not give the webview generic SQL/filesystem access.
- **2026-08-21 — D7:** Store companion outcomes for counts but present music rows by default in the flat detail view.
- **2026-08-21 — D8:** Use React + TypeScript + Vite, i18next, motion-based view transitions, and virtualized history details with a tokenized deep graphite/indigo visual system.
- **2026-08-21 — D9:** Use Tauri IPC channels for run events and raw IPC responses for artwork.
- **2026-08-21 — D10:** Publish one universal macOS DMG and one Windows x86_64 NSIS `.exe`; no raw portable binary, auto-update, or store distribution in v1.
- **2026-08-22 — D11:** Interpret v1 public tags as checksum-backed unsigned preview prereleases, not trusted production releases. macOS uses only an ad-hoc signature and Windows remains `NotSigned`; signing/notarization is a future optional channel and not a T5.6 completion condition. This supersedes the initial protected-signing completion gate.
- **2026-08-21 — D12:** Keep capabilities/CSP narrow and route all filesystem, database, and optional metadata network work through Rust.
- **2026-08-21 — D13:** Name the product `UnlockM` everywhere user-facing. Implement the frontend exclusively through local Vite+ (`rtk vp -C desktop ...`), configured to use pnpm internally; raw package-manager commands are forbidden. This supersedes D1's original raw-npm/package-lock wording and all raw-npm validation examples.
- **2026-08-21 — D14:** Invoke the installed Tauri CLI through `rtk vp -C desktop exec tauri ...` in local validation and CI/release scripts. Bare Tauri CLI invocations would violate D13 even when the executable happens to be locally installed.
- **2026-08-21 — D15:** The supplied `UnlockM Icon.png` is the sole application-icon source. Preserve it unmodified in the repository and derive platform assets with the Vite+-routed Tauri icon command; do not independently redraw an SVG or alter the artwork. If an input-size limitation requires it, a documented reproducible upscale may precede the derivative command while retaining the original source file.
- **2026-08-21 — D16:** Snapshot delivery is an explicit observer capability, not an incidental side effect of core conversion. The no-op CLI/watch path must not re-read output tags or allocate cover bytes; only the desktop history observer may request bounded local snapshots.
- **2026-08-22 — D17:** Keep `react-i18next` 16.5.8 and TypeScript 5.9.3 as the last complete verified pair. The user explicitly prefers a stable validated set over tail-version chasing.
- **2026-08-22 — D18:** Keep Vite+/Vite Core alias/override at 0.2.8 and native Vitest at 4.1.10 through `vp test`. The 0.2.9/4.1.11 trial was fully reverted because it diverged from Vite+'s matching native test shim.
- **2026-08-21 — D19:** Tauri raw IPC `Response` has no custom MIME header. Retain raw artwork delivery without JSON/base64 expansion through `get_artwork`, and expose the already-validated MIME by the paired, narrow `get_artwork_mime` command so the frontend can construct its Blob correctly.
- **2026-08-21 — D20:** Switch Vite+'s internal backend to the current stable pnpm 11.22.0, declared in `packageManager` and locked by `pnpm-lock.yaml`. Invoke it only through `rtk vp -C desktop ...`; raw pnpm commands remain prohibited.
- **2026-08-21 — D21:** Pin CI/release actions to reviewed immutable SHAs: `voidzero-dev/setup-vp` v1.17.0 (`313600b80b104eadebb9111787d37a2e83e014ca`), Tauri Action 1.0.0 (`1deb371b0cd8bd54025b384f1cd735e725c4060f`), checkout v7.0.1, upload-artifact v7.0.1, and download-artifact v8.0.1. Use `setup-vp` rather than setup-node/pnpm actions; frontend lifecycle remains `vp`-only.
- **2026-08-21 — D22:** Grant only `core:event:allow-listen` and `core:event:allow-unlisten` in addition to `dialog:allow-open`: Composer needs them for official native drag/drop subscription cleanup. The capability policy must remain an exact allowlist rather than broad `core:event:default`.
- **2026-08-21 — D23:** Treat the Fontsource OFL obligation as a shipped-artifact contract, not a development-tree note: commit the full SIL OFL-1.1 text, map it and the third-party notice through `bundle.resources`, assert the mapping against the exact bundled font dependencies, and inspect the packaged resource paths.
- **2026-08-22 — D24:** Keep `@types/node` lock-resolved at 24.13.3 despite a newer Node 26 type release because local/CI Vite+ setup intentionally runs Node 24. Revisit when the project’s runtime baseline moves, not merely because an unrelated type-major is available.
- **2026-08-22 — D25:** Add a root `.gitattributes` LF text policy with explicit binary exclusions so Vite+ format checks are platform-invariant. Keep source validation a failing gate, but use a named locked install plus `!cancelled()`/install-outcome conditions for later desktop build/compile steps; successful compilation evidence then remains visible even when a source quality gate fails.
- **2026-08-22 — D26:** Default tag releases to no-secret unsigned preview prereleases with no signing-service or user-managed signing-credential cost. macOS receives only a declared/verified Tauri ad-hoc signature; Windows app/NSIS are declared/verified `NotSigned`; both assets carry `unsigned` names and SHA-256 files. First-open documentation must use narrow per-app platform UX and never recommend disabling system-wide protections. GitHub-hosted runner minutes and artifact storage remain subject to the owner's plan/quota. Future paid signing/notarization is optional and does not block T5.6 completion.

## Completion Evidence

### Complete implementation and local validation

The desktop implementation/configuration, documentation, no-secret unsigned-preview CI/release definitions, licensing resources, and all local validation are complete. The Progress log records the locked 162-test/18-suite workspace evidence, 35 frontend tests, policy/license contracts, Vite+-only workflow guards, BrowserOS visual QA, native macOS debug-app conversion/restart/history QA, and read-only unsigned arm64 plus universal DMG content/checksum smokes. The final local universal app verifies as `Signature=adhoc`, dual-architecture, resource-complete, and sidecar-free; this is intentionally not a notarized/trusted production signature.

### External preview-release gaps (T5.6 remains open)

No real Actions-created unsigned preview prerelease, Windows x86_64 NSIS install/launch run, or published preview artifact hash has been claimed. Those require a tag-triggered workflow run and a Windows host—not signing/notarization credentials. During that external validation, record:

- task/milestone completion dates and commit hashes,
- exact Rust/frontend/native packaging commands and outcomes,
- macOS and Windows artifact names, architectures, sizes, SHA-256 checksums, and bundle-content inspection,
- macOS ad-hoc and Windows `NotSigned` verification output,
- manual QA results for file/folder picker and drag/drop, quick settings, cancellation, partial failure, restart persistence, detail metadata/artwork, both locales, and reduced motion,
- confirmation that source files were retained, outputs were atomically published, metadata-off made no provider request, and no `unlockm` CLI sidecar/process was present,
- review findings and their resolution or explicit acceptance.
