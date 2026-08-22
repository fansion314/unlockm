# Tauri Desktop App for `unlockm`

## Status

In Progress

## Objective

Build a polished Tauri v2 desktop application for macOS and Windows that links the existing `unlockm` Rust library directly into the application, converts one selected file or recursively converts one selected directory, supports native file/folder picking and drag-and-drop, exposes only a concise set of understandable pre-run options, and persists conversion history across application restarts.

The redesigned shell has a fixed localized product bar, an explicit language menu, and a light/dark toggle whose fresh-install default is light. The home view puts the drop target and action/options column in an approximately golden-ratio composition, then places a full-width one-row-per-run conversion history below it. Opening a run transitions to a compact detail view with a server-backed searchable, filterable, sortable, genuinely paginated flat song list. Each song shows artwork, title, artist, album, converted file type and size, provides one-at-a-time in-app playback with play/pause and seeking, and offers narrow actions to reveal the file or open it in the system player.

The v0.2.0 release is multilingual (`zh-CN`, `en-US`, and `ja-JP`), visually modern and restrained in both themes, motion-rich without sacrificing accessibility, and distributed by GitHub Actions for macOS and Windows.

The desktop application must not start the `unlockm` CLI or any other conversion sidecar. All decryption, path planning, metadata handling, bounded concurrency, and atomic publication remain in the in-process Rust core.

## Scope

- Add a Tauri v2 desktop package under `desktop/`, with a React + TypeScript + Vite frontend and a Rust Tauri backend package under `desktop/src-tauri/`.
- Use the user-provided `/Users/peilin/Downloads/UnlockM Icon.png` as the sole source artwork for UnlockM application icons. Preserve an unmodified copy in the repository; do not redraw or replace it with an SVG. Generate all Tauri `.icns`, `.ico`, and PNG derivatives only through `rtk vp -C desktop exec tauri icon ...`, documenting any reproducible upscale required by the CLI.
- Turn the root package into the reusable engine/CLI member of a small Cargo workspace while keeping the existing `unlockm` CLI behavior and release build intact.
- Add a stable, UI-independent batch event/reporting surface to the root Rust library so a desktop host can receive ordered batch and per-file lifecycle data without scraping logs.
- Support exactly one file or exactly one directory per user-initiated run. Directory input remains recursive and uses the existing mirror path planner; “flat detail view” refers only to presentation, not `--flat` output semantics.
- Support native input selection, native output-directory selection, and OS-native file/folder drag-and-drop.
- Offer only these understandable quick settings before conversion:
  - conservatively update metadata and covers (`update_metadata`, off by default),
  - copy ordinary companion files for directory input (`copy_passthrough`, on by default),
  - choose an output directory; default to the input file parent or the input directory, matching the CLI.
- Keep replacement of existing destinations off in the redesigned UI. Keep mirror layout, automatic job limits, streaming buffer size, metadata concurrency, and automatic key discovery as fixed core defaults. Do not expose flat output, manual key material, or implementation terminology such as “atomic replacement” in the primary interface.
- Allow one active conversion run at a time and provide cancellation as a minimum safety control for large directory runs.
- Persist runs, per-path outcomes, technical error/warning details, music metadata snapshots, and deduplicated artwork in a local SQLite database located in Tauri's application-data directory.
- Recover incomplete `planning`/`running` rows as `interrupted` at next startup without touching source or output files.
- Keep newest-first keyset pagination for run history. Replace detail “load more” with SQL-backed page/page-size queries whose search, album filter, artist filter, outcome filter, and sort all apply to the complete run rather than only already-loaded items.
- Provide complete Chinese, English, and Japanese UI resources, a proper language dropdown, localized product subtitles (`UnlockM · 音乐解锁` / `UnlockM · Music Unlocker` / `UnlockM · 音楽ロック解除`), locale persistence/detection, translated product/error-code copy, and sensible filename/path fallbacks.
- Provide complete light and dark token sets, default new users to light, persist an explicit theme choice, and avoid a wrong-theme flash during startup by applying a synchronous cached theme before React mounts and reconciling it with Rust-owned preferences during bootstrap.
- Use one fixed/sticky title bar on both home and detail views, with icon buttons for language and theme plus localized accessible names/tooltips.
- Recompose the home conversion area as an approximately `1.6fr / 1fr` desktop split: the drop/select surface is dominant; the right stack contains a large output-directory control, only applicable quick settings, and a large restrained primary conversion action. Stack responsively in narrow windows.
- Place conversion history below the composer as full-width rows showing input file/directory name, output path, exact music count for terminal history (active progress may retain admitted-item counts), aggregate result, and timestamp. Each row's primary content opens detail, exposes a separate icon action to reveal its output directory, and retains record-only deletion without nesting interactive controls.
- Make the detail summary card shorter and denser while adding known input/output, time, duration, counts, and result facts. Below it, show a flat song list with artwork, title, artist, album, converted output size/type, compact result when needed, and no expandable raw diagnostic section.
- Add a shared one-track-at-a-time audio controller behind per-song play/pause and accessible seek controls. Serve only persisted music-item outputs through an item-ID-based, range-aware Tauri custom protocol; do not expose arbitrary filesystem paths or load whole audio files into SQLite/IPC.
- Add per-song icon actions to reveal the exact converted file in Finder/Explorer and to open it with the platform's default music player. Add the same narrow run-ID-based reveal action for a history row's output directory.
- Implement whole-view home/detail transitions, shared row/header motion, live progress animation, drag-over feedback, and reduced-motion behavior.
- Add component, state/reducer, Rust unit/integration, persistence/restart, and macOS-native build/runtime validation for this redesign. Preserve the existing Windows product code, CI, and release definitions, but do not require new Windows execution evidence in this implementation cycle.
- Add GitHub Actions checks and tagged release packaging for a universal macOS DMG and a Windows x86_64 NSIS `.exe` installer.
- Publish those tag artifacts automatically as clearly labeled unsigned preview prereleases: macOS uses Tauri ad-hoc signing only, Windows uses no Authenticode certificate, all artifacts include SHA-256 files, and documentation gives least-privilege first-open guidance.
- Update `ARCHITECTURE.md`, user documentation, license notices, and build instructions when implementation changes the repository boundary.
- Change the canonical bundle/application identifier from the erroneous `world.fansion.unlockm` to `world.fansionia.unlockm`, and perform a one-time SQLite online backup from the legacy app-data directory when the new history database does not yet exist.
- Rebalance the compact detail summary so long input names/paths and localized completion timestamps are fully readable at the 1080×760 native window instead of being clipped around oversized metric gaps.
- Customize the native About panel with bilingual token/recipient credits and `Copyright © Hamlet Fansion (Peilin Fan)` while preserving the standard macOS application/File/Edit/View/Window/Help menus.
- Populate history display metadata from trusted local NCM container evidence even when `update_metadata` is off; use conservative `Artist - Title` filename parsing only when both final tags and container evidence lack artist/title. This history-only fallback must not cause network access or mutate the converted audio.

## Non-Goals

- Watch mode in the desktop application.
- Running the `unlockm` executable as a subprocess, Tauri sidecar, shell command, or external service.
- Literal single-OS-process operation; Tauri and the platform webview may own implementation processes. The requirement is that conversion has no extra CLI/worker process and that the Rust engine is linked into the app.
- Linux, mobile, App Store, or Microsoft Store distribution in the initial release.
- A raw portable macOS binary outside an `.app` bundle, or a raw portable Windows binary with no installer/runtime assumptions. The release contract is one downloadable installer artifact per platform.
- Paid/protected-code-signing, macOS notarization, Windows Authenticode reputation, and a claim that preview artifacts are trusted production releases. A future signed channel may be added separately without changing the unsigned-preview contract.
- Waveform visualization, audio transcoding for preview compatibility, playlists, queue management, library management, search across separate runs, history export/import, cloud sync, accounts, telemetry, auto-update, or background/tray operation.
- Multiple simultaneous batch runs, multi-selection of unrelated roots, or queuing several roots from one drag operation.
- Exposing `--watch`, `--flat`, `--jobs`, `--buffer-size`, `--metadata-jobs`, QMC ekeys/maps/MMKV passwords, or KGM database paths in the initial UI.
- Audio transcoding, deleting encrypted sources, changing collision policy, weakening atomic publication, or changing metadata confidence rules.
- Fetching metadata or artwork solely to decorate history. History captures the final/local output state; network access remains gated by `update_metadata`.
- Theme customization beyond the explicit light/dark pair, automatic system-theme following, or locales beyond `zh-CN`, `en-US`, and `ja-JP` in v0.2.0. The token and resource structure must remain extensible.
- A generic webview filesystem/opener capability, arbitrary-path audio URL, or arbitrary-path “open with system application” command. All playback/reveal/open operations must resolve a persisted run/item ID in Rust and revalidate the recorded destination.

## Current Context

- The implemented baseline is at `b9701c3` on the current branch. This file remains the only active desktop/Tauri ExecPlan; M1–M5 baseline work is implemented and reviewed except for the external T5.6 preview/Windows evidence, while M6–M9 are newly planned and open.
- The root Rust 2024 `unlockm` package and `desktop/src-tauri` workspace child are implemented. The desktop links the library in process; `RunOptions`, bounded structured `BatchObserver` reports, cancellation, collision planning, source retention, and atomic publication are shared with CLI/watch.
- `src/metadata/tags.rs::read_bounded_snapshot` content-probes final audio and supplies title, album, artist, duration, and bounded artwork only to observers that opt into snapshots. It is the correct seam for adding actual published output length/type without adding a TypeScript parser or changing the no-op CLI/watch path.
- `src/app/scan.rs` is recursive, rejects symlinks, prunes an output subtree inside the input, and uses the same planner as the CLI. These containment and source-retention semantics remain authoritative.
- `desktop/src-tauri/src/history/mod.rs` owns schema-v1 SQLite runs/items/artwork/preferences, restart recovery, record-only deletion, keyset run/item reads, and artwork dedupe. `desktop/src-tauri/src/commands/mod.rs` keeps database and conversion work off the Tauri main thread and exposes typed commands/DTOs.
- `desktop/src-tauri/capabilities/default.json` currently grants only drag-event listen/unlisten and dialog open. `tauri.conf.json` has no asset protocol or opener capability. Policy tests already reject generic process/sidecar/filesystem/database access and must be extended, not relaxed wholesale.
- `.github/workflows/ci.yml` and `desktop-release.yml` already cover locked Vite+/Rust desktop builds and the no-secret unsigned-preview packaging contract. `ARCHITECTURE.md`, both READMEs, third-party notices, and bundled license resources describe the implemented baseline and will need incremental redesign updates.
- The user-supplied icon is preserved at `desktop/src-tauri/icons/source/UnlockM Icon.png`, and the existing generated Tauri derivatives remain authoritative; the redesign does not redraw or replace it.
- Previously verified Tauri v2 references remain relevant for commands, IPC, dialog, drag/drop, and distribution:
  - <https://v2.tauri.app/develop/calling-rust/>
  - <https://v2.tauri.app/develop/calling-frontend/>
  - <https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/>
  - <https://v2.tauri.app/plugin/dialog/>
  - <https://v2.tauri.app/distribute/>
  - <https://v2.tauri.app/distribute/pipelines/github/>
- `desktop/src/App.tsx` now composes the fixed shared `AppHeader`, 1.6:1 `Composer`, and full-width history below it. The primary row-content button opens detail; sibling buttons reveal output and delete only the history record.
- `desktop/src/features/conversion/Composer.tsx` now contains only operational drop/select/output/settings/action content. The overwrite control is removed and both preferences/request construction normalize it to the safe `false` default; metadata refresh and directory-only companion copy remain.
- `desktop/src/styles/tokens.css` now defines complete light-default and dark semantic tokens. A synchronous local cache paints before React, then the schema-compatible Rust preference reconciles locale/theme/settings once at bootstrap.
- `desktop/src/features/history/HistoryDetail.tsx` now queries true 25-song pages with stale-response protection, search, searchable album/artist facets, outcome filter and allowlisted sort. Rows show artwork/metadata/output facts, no raw diagnostics, one shared in-app player with seek, and separate Finder/system-player actions.
- SQLite schema version 2 stores nullable actual output bytes/content format, while bounded schema-v1 page reads repair only revalidated destinations. `query_run_items` filters/sorts/counts before pagination and `RunSummary.music_count` no longer relabels companion-inclusive totals.
- `desktop/src-tauri/src/audio_protocol.rs` implements the item-ID-only bounded media protocol; `commands/mod.rs` exposes ID-only native actions through opener 2.5.4. The capability file remains dialog/event-only and the generic asset/opener surfaces stay unavailable to the webview.
- Follow-up inspection found the canonical identifier is misspelled as `world.fansion.unlockm`; the corrected identifier changes the Tauri app-data directory, so history continuity requires a legacy database backup rather than merely changing `tauri.conf.json`.
- Native 1080×760 detail evidence shows the summary's title/path column and localized completion timestamp clipped while the four fact columns reserve excessive space between values.
- `snapshot_music` currently reads only final output tags. NCM container JSON is parsed only inside the optional tag-update pipeline, so metadata-off conversions can persist a correct album/title yet still lose the known artist in desktop history.

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
│   ├── i18n/locales/{zh-CN,en-US,ja-JP}.json
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

The redesigned action column shows one large output-directory button above only the settings that have clear user value. `update_metadata` uses concise product copy and an accessible tooltip for the optional verified network request; `copy_passthrough` is visible only for directory input. `overwrite` is no longer editable in the primary UI and new frontend requests always send its safe `false` default. Preserve the backend field and existing historical snapshots for compatibility rather than rewriting old records.

Persist locale, explicit `light`/`dark` theme, and the remaining quick-setting defaults as app preferences, but snapshot actual conversion settings into every run record so history remains truthful if defaults later change. Fresh preferences default to `light`; deserializing a pre-theme preferences value also defaults to `light`. A small localStorage theme cache may be used only to avoid first-paint flashing and must be reconciled with the Rust-owned preference on bootstrap.

Only one run may be active. Starting another returns a typed `busy` error. Cancellation uses the existing core cancellation token, never deletes sources, never publishes an incomplete private stage, and yields a terminal `cancelled` run status after workers stop.

### D6. Desktop command and event boundary

Expose a small command set from `desktop/src-tauri/src/commands/`:

- `bootstrap`: current preferences, active-run summary if any, and the first history page.
- `validate_input`: authoritative file/directory validation for picker/drop UX.
- `start_conversion(request, on_event: Channel<ConversionEventDto>)`: create the persistent run before work, mark it active, stream ordered progress, and resolve with the terminal run summary.
- `cancel_conversion(run_id)`: idempotently request cancellation only for the matching active run.
- `list_runs(cursor, limit)`: newest-first keyset pagination.
- `query_run_items(run_id, query)`: one true page of flattened music items plus filtered/total counts and companion counts. `query` contains a clamped page size, page index, trimmed song search, optional artist/album/outcome filters, and an allowlisted sort/direction; filtering and sorting execute in SQLite before pagination.
- `list_run_item_facets(run_id, facet, search, cursor, limit)`: bounded, deduplicated non-empty artist/album values for accessible searchable filter popovers rather than loading an unbounded facet list into the webview.
- `get_artwork(artwork_id)` plus `get_artwork_mime(artwork_id)`: return raw bytes and the validated MIME through two narrow Tauri IPC calls, because Tauri's raw `Response` body does not carry a custom MIME header; the frontend creates/revokes object URLs and lazy-loads visible covers.
- `reveal_run_output(run_id)`: resolve only the stored run output directory in Rust, reject a missing/invalid/symlink target, and reveal/open that directory through the platform opener integration.
- `reveal_run_item(item_id)`: resolve only a persisted music item's destination in Rust, revalidate a regular non-symlink file, and reveal that exact file in Finder/Explorer.
- `open_run_item(item_id)`: perform the same item-ID lookup and validation, then ask the operating system to open the converted file with its default audio application.
- `delete_run(run_id)` and `clear_history`: remove history/artwork references only after explicit confirmation and never touch source/output files.
- `get_preferences` / `set_preferences`: locale, explicit theme, and quick-setting defaults.

Use a Tauri IPC `Channel` for ordered run events instead of global event names. DTOs use camelCase at the IPC boundary while core and database types remain idiomatic Rust. Generate or manually maintain one TypeScript contract module and add serialization contract tests to prevent silent field drift.

### D7. Persistent history and crash recovery

Use a Rust-owned `rusqlite` store in the Tauri application-data directory. Do not expose a generic SQL plugin or database path to the webview. Initialize the directory and schema during Tauri setup before the main view leaves its bootstrap state. Use `PRAGMA user_version` migrations, foreign keys, WAL mode, bounded busy timeout, and explicit transactions.

Initial logical schema:

| Table | Purpose and required fields |
| --- | --- |
| `runs` | Integer id, input kind/path, output path, settings snapshot, status, timestamps, total/completed counts, all `Summary` counters, app/schema version, representative artwork id, terminal error code/detail. |
| `items` | Integer id, run id, stable ordinal, item kind, source/relative/destination paths, action, outcome, source logical bytes, nullable actual output bytes and content-probed output format, timings, error code/detail, warning JSON, title, artist, album, duration, artwork id. |
| `artwork` | Integer id, SHA-256 digest unique key, validated MIME, original bytes, byte length, created timestamp. Identical covers are stored once. |
| `preferences` | Key/value settings with schema-versioned JSON for locale, explicit light/dark theme, and quick-setting defaults. |

Write run/item transitions through one blocking database coordinator. Commit terminal item state before forwarding the corresponding terminal UI event so a webview reload cannot display progress that is absent from history. Add an idempotent schema-v2 migration for nullable `output_bytes` and `output_format` plus query indexes scoped by `run_id`/`item_kind`; do not scan every historical output during startup. Derive terminal `music_count` for `RunSummary` from the indexed item rows rather than relabeling `total_items`, because the latter includes companions. For schema-v1 rows, repair missing output facts only for the bounded page being read and only when the recorded destination still revalidates, then persist the repair best-effort. Query commands run on blocking workers and never block the Tauri main thread.

Detail queries use allowlisted SQL fragments for sort columns/direction and bound parameters for search/facets. The response returns exact filtered and total counts so page controls remain correct after filters change; changing search/filter/sort resets to page 1. Tests must prove filters apply across unloaded rows and page boundaries remain stable under every supported sort.

At startup, atomically convert stale `planning` or `running` runs to `interrupted`, set a recovery timestamp/message, and retain completed item records. Atomic core publication means an item may already have a valid output even if the process stopped before recording its terminal report; v1 does not guess or rescan to upgrade that record.

Deletion uses foreign keys and a post-delete orphan-artwork cleanup transaction. No operation recursively deletes files from user-selected paths.

### D8. Music metadata and artwork snapshots

Extend terminal reporting so successfully converted audio reads its final destination through `metadata::tags::read` after publication. The same bounded snapshot records the actual final file length and a normalized content-probed container label (`MP3`, `FLAC`, `M4A`, and so on), never trusting only the filename suffix. A safely skipped audio destination may also be read if it exists and content-probes as supported audio. Failed/conflicted items use a filename fallback and placeholder artwork; they remain visible with a compact localized outcome but no expandable raw diagnostics in the product list.

Do not show ordinary copied companions as music rows. Persist them for accurate counts/diagnostics with `item_kind = companion`, while the detail query returns music items by default and a compact companion summary. Encrypted candidates that fail before destination planning remain music items when their registered suffix identifies them as such.

Copy only normalized display fields and validated cover bytes into the history snapshot. Never store encrypted or decrypted audio payloads in SQLite. Deduplicate artwork by digest, enforce the core's existing 10 MiB cover ceiling, load bytes lazily, and revoke frontend object URLs when rows leave the viewport. Missing/corrupt artwork degrades to a deterministic local placeholder.

### D9. Product UI, visual direction, and motion

Use React function components, a typed reducer/context for conversion state, and feature-local hooks. Avoid a broad state-management framework in v1. Use `motion/react` for shared layout/view transitions and retain `@tanstack/react-virtual` only where it still adds value inside a bounded page. Use one consistent icon set rather than Unicode glyphs; icon-only controls require localized accessible names, visible focus, and tooltips. Keep all light/dark color, typography, spacing, radius, elevation, duration, and easing values in CSS custom-property tokens.

Visual direction: light mode is the fresh-install default with warm off-white/neutral surfaces, restrained indigo/teal accents, dark readable text, and album artwork as the richest color source. Dark mode adapts the existing graphite/indigo character without the current gold primary-button glare. Both themes use crisp opaque surfaces, minimal gradients, compact native-app density, generous hit targets, the supplied UnlockM lock/music-mark, and visible keyboard focus. Avoid indiscriminate blur, decorative microcopy, oversized marketing headlines, and yellow/gold conversion controls.

Shared shell:

- a fixed/sticky top bar that does not scroll away on home or detail,
- `UnlockM · 音乐解锁` in Chinese and `UnlockM · Music Unlocker` in English,
- a globe icon opening an explicit three-language menu with checked selection and full keyboard/menu dismissal behavior,
- a sun/moon icon toggling persisted light/dark mode,
- icon-only controls wherever the symbol is conventional, with text retained where an icon alone would be ambiguous.

Home view:

- an approximately golden-ratio conversion composition, with the larger left drop/select surface and smaller right action stack,
- a large output-directory chooser at the top of the right stack, only applicable quick settings beneath it, and a restrained indigo/teal primary Convert action at the bottom,
- no duplicate eyebrow/title/description copy around the composer; preserve only labels, selected path, actionable validation, and status needed to operate it,
- a full-width conversion history below the composer, with one readable row per run showing file/folder icon or representative art, source basename, output path, exact terminal music count, aggregate result, and timestamp,
- separate sibling icon buttons for row detail, output-directory reveal, and record deletion so clicking one action cannot trigger another,
- empty, bootstrap, validation, busy, and history-error states designed explicitly.

Detail view:

- whole-view slide/fade transition from home with reverse back navigation,
- shared layout transition from the selected history row into a run summary header,
- a shorter summary card showing source, output, start/end or duration, result counters, and output-reveal action without repeating the page title in oversized type,
- a control row with song search, searchable artist and album facets, optional result filter, allowlisted sort, and a clear-all action; Chinese labels must use “歌曲/全部歌曲”, never “首音乐” as a standalone label,
- true previous/next and numbered page navigation with a bounded default page size of 25; skeleton and empty states do not shift the layout,
- flat song rows with cover, title, artist, album, converted format, human-readable converted size, and compact outcome only when it adds information; raw source/output/error/warning diagnostics are not rendered,
- one shared audio element/controller so only one item plays at a time, with per-row play/pause, elapsed/duration text, and keyboard/pointer-accessible seek slider,
- per-row icon actions to reveal the converted file and open it in the system player; missing/moved/unsupported files fail locally with translated non-destructive feedback.

Motion guidelines: 180–320 ms for primary transitions, spring/layout motion only where it clarifies continuity, no perpetual decorative animation, and no animation that blocks conversion feedback. Under `prefers-reduced-motion`, remove travel/stagger/parallax and retain only near-instant opacity/state cues. Clean up all event listeners, channels, timers, and artwork URLs on unmount.

### D10. Internationalization and error presentation

Use `i18next` + `react-i18next` with complete `zh-CN`, `en-US`, and `ja-JP` resources. Detect Chinese and Japanese platform/browser locales on first launch, fall back to English for unsupported locales, and persist explicit user selection. The title bar's localized product subtitle changes with locale. Keep translation keys semantic and prohibit hard-coded user-facing strings in feature components.

Rust commands return stable error codes plus interpolation parameters and a separate technical detail. The frontend translates the product message; an app-level failure banner may retain a collapsed technical detail for support, but song rows never expose raw diagnostics. Stored history retains both the code and original technical detail so later app versions can retranslate the product copy.

Test all three locales for missing keys, interpolation, plural/count text, long English layout, Chinese/Japanese punctuation, path truncation, and reduced-motion behavior. Dates/times and durations use `Intl` formatting, not hand-built locale strings.

### D11. Packaging and release interpretation

“Single executable” is interpreted as both:

1. the app contains the conversion engine and ships no `unlockm` sidecar or required FFmpeg/helper binary; and
2. each platform release offers one primary downloadable installer file.

Publish a universal Apple Silicon + Intel macOS DMG containing a Tauri ad-hoc-signed `.app`, and one unsigned Windows x86_64 NSIS setup `.exe`. Both are explicitly labeled unsigned previews and accompanied by SHA-256 files. The installed macOS application remains an `.app` bundle and Windows remains an installed application because those are platform conventions. Windows may bootstrap WebView2 when absent; embedding the roughly 127 MiB offline WebView2 installer is not the default unless offline installation becomes an explicit requirement.

Add a tag-driven `.github/workflows/desktop-release.yml` using the official `tauri-apps/tauri-action`. Add a non-release desktop build/check workflow or extend CI so both target OSes compile the Tauri crate and frontend before release. Release jobs must use locked Rust and Vite+-resolved frontend dependencies, upload only the intended primary artifacts plus checksums, and fail if a CLI sidecar/resource appears in the bundle.

The tag workflow must require no GitHub environment, identity, certificate, password, or signing secret. It must set `APPLE_SIGNING_IDENTITY='-'` for macOS ad-hoc signing, verify that specific signature mode without notarization/stapling, and assert that both the Windows app executable and NSIS installer are `NotSigned`. Release title/body and filenames must say `unsigned preview`; documentation must explain SHA-256 verification and the narrow platform-native first-open flow without asking users to disable Gatekeeper, SmartScreen, or other global protections.

### D12. Security and privacy boundary

Grant only the Tauri dialog permission and core window/event permissions actually used. Do not expose shell, generic filesystem, SQL, HTTP, updater, clipboard, generic opener, or arbitrary-path commands to the webview. Rust owns all filesystem/database/network and OS-open resolution, accepts only run/item IDs for reveal/open actions, and revalidates the stored destination as an expected regular non-symlink file or directory immediately before use. Use the latest stable compatible official `tauri-plugin-opener` only from these Rust commands; grant none of its generic frontend permissions. The opener is never part of conversion and does not weaken the no-CLI-sidecar invariant.

Register a dedicated item-ID audio protocol in Rust. It resolves `/item/<id>` through history, validates the destination, content-probes an allowlisted audio type, supports `HEAD` and bounded byte-range responses needed for seeking, and never accepts a filesystem path. Set a restrictive CSP that permits packaged frontend assets, Tauri IPC, local `blob:` artwork URLs, and only this protocol's platform-specific media origins (`unlockm-audio:` and `http://unlockm-audio.localhost`) in `media-src`; do not enable the generic asset protocol. Album art is delivered from stored bytes, never rendered from a provider URL. Keep metadata networking inside the existing allowlisted Rust provider and only when `update_metadata` is true.

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
- Dependencies: baseline implementation depends on M1–M4; final T5.6 evidence also depends on completed/reviewed M6–M9 so released artifacts exercise the redesigned product.
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

### M6. Queryable output facts and safe native file actions

- Goal: provide the truthful output metadata, whole-run query semantics, item-scoped media delivery, and OS file actions required by the redesigned UI without broadening the webview's filesystem authority.
- Dependencies: implemented M1–M4 history/event/IPC boundaries.
- Acceptance criteria:
  - Newly terminal music items persist actual published file size and content-probed format; schema-v1 history remains readable and bounded page reads may repair still-valid destinations.
  - Search, artist/album/outcome filters, allowlisted sorting, exact counts, and page navigation execute before pagination against the complete run.
  - Audio URLs contain only item identity, support bounded byte ranges and seeking, and reject missing, non-music, failed, unsupported, symlinked, or unknown items without exposing paths.
  - Run-output reveal, item reveal, and system-player open commands accept IDs only, revalidate the stored target, and expose no generic opener/shell/filesystem capability.
- Validation:
  - `rtk cargo test -p unlockm-desktop --all-targets`
  - Focused schema-v1→v2, filtered-pagination, facet-bound, path-revalidation, protocol `HEAD`/range/invalid-ID, reveal/open adapter, and DTO contract tests.
  - `rtk cargo clippy -p unlockm-desktop --all-targets -- -D warnings`

### M7. Fixed bilingual shell and home-view redesign

- Goal: match the supplied home-view composition with a fixed localized title bar, light-default theme pair, dominant drop target, compact action column, and full-width conversion history.
- Dependencies: M6 for history-row output actions; existing composer/run state.
- Acceptance criteria:
  - A fresh profile paints light mode by default; explicit dark/light and language selections persist across restart without a wrong-theme startup flash.
  - The fixed header shows the localized product subtitle and accessible language-menu/theme icon controls on both routes.
  - At desktop widths the drop/action columns approximate `1.6:1`, with output chooser first and Convert last; the layout stacks without clipping at the supported narrow width.
  - Explanatory marketing/microcopy and the overwrite option are absent from the primary conversion area; metadata and directory companion copy remain functional.
  - Conversion records render below the composer as one full-width row with source/output identity, count, result, time, detail, output-reveal, and record-delete actions.
- Validation:
  - Frontend preference/bootstrap, menu keyboard, theme, composer request, history-row action-isolation, and responsive-state component tests.
  - `rtk vp -C desktop check`
  - `rtk vp -C desktop test`
  - Visual QA at 920×640, 1024×700, and 1280×800 in both locales/themes, including scroll proof that the header remains fixed.

### M8. Searchable paginated song detail and playback

- Goal: deliver the compact run summary and information-dense flat song list requested in the sketch, including true pagination, filters, in-app playback, and native per-file actions.
- Dependencies: M6 query/protocol/actions and M7 shared shell/theme tokens.
- Acceptance criteria:
  - The compact summary shows known run facts without consuming excessive vertical space.
  - Search, artist/album/result filters, sort, clear, and numbered previous/next pagination remain mutually consistent and operate on all matching rows, not just a loaded prefix.
  - Song rows show artwork, title, artist, album, actual converted size/type, and only concise product outcomes; no expandable technical diagnostics are visible.
  - Exactly one item plays at a time; play/pause and seek work with keyboard and pointer, switching tracks stops the previous item, and navigation/unmount releases playback resources.
  - Each playable row can reveal the exact file or open it with the system player; unavailable files produce translated feedback without breaking the list.
- Validation:
  - Frontend query-state, debounce/stale-response, pagination-reset, facet, formatting, shared-player, seek, cleanup, missing-file, and icon-action accessibility tests.
  - Rust/webview integration smoke with real MP3/FLAC fixtures for range playback and seeking on macOS.
  - `rtk vp -C desktop check`
  - `rtk vp -C desktop test`
  - `rtk vp -C desktop build`

### M9. Architecture, regression, and macOS redesign closure

- Goal: integrate the redesign without regressing conversion safety, privacy, accessibility, or the established unsigned-preview packaging contract.
- Dependencies: M6–M8.
- Acceptance criteria:
  - `ARCHITECTURE.md` and user docs describe theme semantics, query pagination, item-ID media delivery, and the explicit user-triggered OS opener exception while retaining the no-conversion-sidecar invariant.
  - Capability/CSP policy tests reject generic filesystem/opener/asset/shell access and allow only the exact new media origins and Rust commands.
  - The macOS webview passes the home/detail functional and visual matrix in both locales/themes; long lists remain bounded and scrolling/playback stay responsive.
  - Windows product/CI/release definitions remain present, but Windows runtime, installer, and visual QA are explicitly outside this cycle's completion gate per user direction.
  - T5.6 macOS package/install evidence is gathered only after the redesign tasks and review findings are resolved.
- Validation:
  - Full workspace/frontend validation and platform-native Tauri builds from the Build and release gates below.
  - Manual light/dark, locale, drag/drop, conversion, history, search/filter/page, playback/seek, reveal, system-open, restart, missing-file, and reduced-motion matrix on macOS.
  - Reinspect packaged capabilities/resources/processes and confirm no `unlockm` CLI sidecar or generic asset/opener permission was introduced.

### M10. Product identity, About credits, detail fit, and artist correctness

- Goal: close the four user-reported production issues without regressing persisted history, metadata-off privacy, or the completed desktop interaction model.
- Dependencies: completed M6–M9 implementation and review closure.
- Acceptance criteria:
  - Built bundle/Info.plist and app-data paths use `world.fansionia.unlockm`; when only the legacy database exists, an online SQLite backup initializes the new path once without deleting or mutating the old copy.
  - At the native 1080×760 window, the detail title/source/output block and all four fact values are readable without ellipsis; gaps are visibly tighter and responsive fallback still avoids horizontal overflow.
  - The native About panel shows the requested bilingual credit naming Additive Cookie, Moxianbao, and vincentlee73 plus `Copyright © Hamlet Fansion (Peilin Fan)`.
  - A metadata-off NCM conversion persists the known container artist/title/album/duration for history; final embedded tags remain preferred when container data is unavailable, and a strict `Artist - Title` filename fallback prevents avoidable “未知歌手” without network or tag mutation.
- Validation:
  - Focused identifier/legacy-backup/About metadata tests and NCM snapshot precedence/no-network regressions.
  - Full Rust/frontend gates and macOS Tauri app/universal DMG build.
  - Native macOS screenshots for adjusted detail and About panel, plus a real metadata-off conversion proving a non-empty artist.

### M11. Japanese localization and v0.2.0 release

- Goal: add complete Japanese product copy and publish the resulting desktop release as `v0.2.0` through the existing no-secret unsigned-preview workflow.
- Dependencies: completed and reviewed M1–M10 implementation, current release workflow, and both configured Git remotes.
- Acceptance criteria:
  - `ja-JP` has exact semantic-key parity with `zh-CN` and `en-US`, natural Japanese product/error copy, Japanese first-launch detection, persistence, and full keyboard-accessible language-menu behavior.
  - The Rust/TypeScript bootstrap contract advertises all three locales without widening any unrelated IPC or capability surface.
  - Root Cargo, desktop Cargo, frontend package, Tauri configuration, and workspace lockfile consistently report `0.2.0`; the `v0.2.0` tag resolves to the validated release commit.
  - `main` and `v0.2.0` are pushed to GitHub, the tag workflow completes, and the GitHub prerelease exposes the exact macOS/Windows unsigned installers plus SHA-256 manifests.
- Validation:
  - Focused locale parity/detection/menu/persistence and Rust/TypeScript bootstrap contract tests.
  - Full Rust/frontend build and release gates listed below.
  - Remote branch/tag identity, GitHub Actions conclusion, prerelease state/title, exact asset list, and checksum metadata inspection.

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
- [x] T5.6 After T6.1–T9.4 are complete, perform final macOS install/launch/conversion/restart/history/playback/native-file-action/manual-motion QA, inspect the macOS bundle for absence of `unlockm` sidecars and unexpected permissions, record exact commands/artifact hashes/results, and resolve review findings before marking the plan `Done`. Windows runtime/installer QA is explicitly waived for this cycle.

- [x] T6.1 Extend `desktop/src-tauri/src/history/Preferences` and `desktop/src/lib/contracts.ts` with an explicit light/dark theme whose serde/backward-compatible default is light; add preference reopen and schema-v1 JSON compatibility tests.
- [x] T6.2 Extend the observer-only music snapshot and persistence DTOs with actual published `output_bytes` and content-probed `output_format`; add SQLite schema-v2 nullable columns, indexes, bounded per-page legacy repair, and fixture coverage for converted, skipped, missing, and renamed outputs without changing CLI/watch allocation behavior.
- [x] T6.3 Replace cursor-only `get_run_items` with typed `query_run_items` plus bounded `list_run_item_facets` in `desktop/src-tauri/src/history/`, `commands/`, `desktop/src/lib/contracts.ts`, and `ipc.ts`; add indexed exact `musicCount` to terminal run summaries without relabeling companion-inclusive `totalItems`; verify search/artist/album/outcome/sort/page/count semantics across unloaded rows and reject non-allowlisted query values.
- [x] T6.4 Add the latest stable compatible official `tauri-plugin-opener` to `desktop/src-tauri/Cargo.toml`, wrap its Rust API behind a narrow adapter, and add ID-only `reveal_run_output`, `reveal_run_item`, and `open_run_item` commands; revalidate stored file kinds/symlinks immediately before use, mock the native side effect in tests, and keep all generic opener/shell/filesystem frontend permissions absent from `desktop/src-tauri/capabilities/default.json`.
- [x] T6.5 Register an `unlockm-audio` item-ID custom protocol under `desktop/src-tauri/`, implement content-type, `HEAD`, `Accept-Ranges`, bounded single-range `206`, `416`, and missing/unsupported/symlink rejection behavior, and update CSP/policy tests for only the custom media origins without enabling `assetProtocol`.

- [x] T7.1 Refactor `desktop/src/styles/tokens.css`, `index.css`, and theme bootstrap code into complete light-default and dark token sets; apply the cached theme before React mount, reconcile it from Rust preferences, and test fresh/migrated/restart behavior plus contrast/focus tokens.
- [x] T7.2 Add the latest stable compatible `@phosphor-icons/react` through the Vite+-only workflow, then build a shared fixed `AppHeader` with the supplied app icon, localized `UnlockM · 音乐解锁` / `UnlockM · Music Unlocker` title, accessible language dropdown, and Phosphor globe/sun/moon controls; test focus order, checked language, Escape/outside dismissal, persistence, and stickiness on home/detail.
- [x] T7.3 Recompose `desktop/src/features/conversion/Composer.tsx` into an approximately `1.6fr / 1fr` drop/action layout, put the large output-directory control first and restrained primary action last, remove redundant explanatory copy and Unicode glyphs, hide overwrite while sending `false`, and retain metadata plus directory-only companion-copy request semantics.
- [x] T7.4 Move `history-panel` below the composer and redesign it as full-width single-line records with input basename/kind, output path, exact terminal music count, aggregate result, timestamp, representative art, a primary row-content detail target, run-output reveal, and record deletion; preserve live progress/cancel, pagination, skeleton/error/empty states, sibling-control semantics, and narrow-window wrapping.

- [x] T8.1 Compact `desktop/src/features/history/HistoryDetail.tsx` summary card and add known source/output, start/finish/duration, counters/result, and output-reveal facts without reintroducing raw diagnostics or oversized title treatment.
- [x] T8.2 Add debounced song search, searchable artist/album facets, optional result filter, allowlisted sort, clear-all, and true numbered previous/next pagination backed by `query_run_items`; cancel/ignore stale responses, reset page on query changes, and preserve stable skeleton/empty/error layouts.
- [x] T8.3 Redesign each song row to show lazy artwork, title, artist, album, converted format, human-readable converted size, and only a concise exceptional outcome; remove expandable source/output/error/warning diagnostics and correct all “首音乐” copy to contextually grammatical “歌曲/全部歌曲/{{count}} 首歌曲”.
- [x] T8.4 Add one detail-owned audio controller and per-row play/pause, elapsed/duration, and seek controls using item-ID protocol URLs; prove single-track ownership, seek/range behavior, track switching, ended/error states, keyboard slider semantics, and cleanup on page/filter/navigation/unmount.
- [x] T8.5 Add per-row icon buttons for exact-file reveal and system-player open, with localized tooltips/accessible names, click isolation, unavailable-file feedback, and disabled/loading states; do not make external system playback part of in-app player state.
- [x] T8.6 Complete both locale resources and component tests for the redesigned shell/detail, including long Chinese/English text, paths, byte/type formatting, light/dark themes, reduced motion, icon-only controls, and page/facet/player states.

- [x] T9.1 Add end-to-end Rust/frontend contract and regression fixtures covering a large multi-artist/multi-album run, schema-v1 history, filtered later-page matches, output facts, playback range requests, reveal/system-open adapters, missing files, and unchanged record-only deletion/source retention.
- [x] T9.2 Update `ARCHITECTURE.md`, `README.md`, `README.zh.md`, third-party notices, and policy checks for the new icon dependency if used, light/dark preferences, SQL-backed detail queries, custom media protocol, and explicit user-triggered OS reveal/open boundary.
- [x] T9.3 Run the full validation matrix and visual QA at supported window sizes in both themes/locales on macOS; record exact commands, screenshots/findings, playback/seek and Finder/system-player evidence, memory/DOM bounds, and the explicit Windows-test waiver in this plan.
- [x] T9.4 Perform implementation review against this updated ExecPlan, fix all material findings, rerun affected validation, and record review closure before starting T5.6 external preview-release evidence.

- [x] T10.1 Change all bundle/app-data documentation and configuration to `world.fansionia.unlockm`; add a schema-safe online SQLite legacy-history backup with first-run/idempotence tests.
- [x] T10.2 Rebalance `detail-header`, `detail-title-block`, and `detail-facts` CSS so long names/paths and localized timestamps display fully at 1080×760 and narrow responsive layouts remain bounded.
- [x] T10.3 Build the standard native menu with custom About metadata containing bilingual credits for Additive Cookie, Moxianbao, and vincentlee73 and `Copyright © Hamlet Fansion (Peilin Fan)`; add metadata/menu contract tests.
- [x] T10.4 Merge trusted local NCM container metadata into observer-only history snapshots regardless of `update_metadata`, add conservative `Artist - Title` fallback, and prove metadata-off artist correctness without network/tag mutation.
- [x] T10.5 Review T10.1–T10.4, fix findings, re-review, rerun full gates/native macOS QA, and replace final DMG hash/evidence before returning this plan to `Done`.

- [x] T11.1 Add complete `desktop/src/i18n/locales/ja-JP.json`, register/detect `ja-JP`, add it to the accessible language menu, and extend parity/interpolation/menu/persistence tests.
- [x] T11.2 Extend Rust and TypeScript bootstrap locale contracts and fixtures to advertise `ja-JP`; run the focused contract suites.
- [x] T11.3 Bump all authoritative package/application versions and workspace lock entries from `0.1.0` to `0.2.0`; verify the release tag guard accepts only `v0.2.0`.
- [x] T11.4 Run full Rust/frontend validation, review the M11 diff against this ExecPlan, fix all material findings, and return the plan to `In Progress` for publication after re-review passes; `Done` still requires T11.5 remote evidence.
- [ ] T11.5 Create the validated conventional commit and annotated `v0.2.0` tag, push `main` and the tag to GitHub, then record the completed Actions prerelease URL, conclusion, assets, and remote commit/tag identity.

## Validation Strategy

### Core regression and contracts

- Preserve all current CLI fixture, layout, metadata, failure-isolation, logging, progress, and watch tests.
- Add table-driven tests ensuring one terminal report per admitted path and exact agreement between report outcomes and `Summary` counters.
- Test destination/error/warning presence for preflight failure, collision, skip, copy, conversion, metadata fallback, and cancellation.
- Test metadata snapshot extraction from extensionless staged/final audio by content, not filename extension, and prove it does not initiate provider traffic when metadata update is off.
- Test observer-enabled output snapshots capture final published byte length and content-probed container format for converted/skipped music while the default CLI/watch observer remains allocation-free and behavior-compatible.
- Test the typed config builder against existing CLI defaults and invalid combinations.

### Persistence and coordinator

- Use temporary app-data directories and real bundled SQLite for migration, reopen, crash recovery, pagination, artwork deduplication, delete/orphan cleanup, and preference tests.
- Assert database terminal state commits before the matching terminal IPC event is delivered.
- Simulate a frontend channel closing mid-run: conversion must remain safe, persistence must complete or the run must cancel with an explicit app-level error, and active state must always clear.
- Simulate process restart with an active row and completed items; only the run status changes to `interrupted`.
- Validate that history deletion changes only the app-data database and never stats/removes recorded source/output paths.
- Migrate a copied schema-v1 database to v2, preserve every run/item/artwork/preference, and verify theme deserialization defaults to light when the old JSON has no theme.
- Populate thousands of multi-artist/multi-album items and prove trimmed search, each filter combination, allowlisted sort/direction, exact filtered totals, stable page boundaries, and bounded facet reads operate across the full SQL result rather than a frontend prefix.
- Verify actual output facts are captured for new items and legacy repair touches only the requested bounded page; missing/moved outputs remain nullable rather than blocking detail reads.
- Exercise item-ID audio protocol `HEAD`, valid/suffix/open-ended range, chunk cap, `416`, MIME, unknown ID, non-music, nonterminal/missing, unsupported, and symlink cases without returning a recorded path in response bodies.
- Mock the native opener adapter and prove run/item commands derive their targets only from history IDs, distinguish directory reveal/file reveal/system open, and reject changed filesystem types before the side effect.

### Frontend

- Unit-test request construction and event reducers independently of React.
- Component-test picker/drop validation, applicable quick-setting visibility/defaults, busy/cancel states, full-width one-row-per-run history, action isolation, navigation, output reveal, true detail pagination, search/facets/sort, compact outcomes, native item actions, and deletion confirmation with mocked Tauri APIs.
- Run all tests once per locale and enforce a no-missing-translation-key check.
- Mock artwork raw responses and assert lazy load, object URL revocation, placeholder fallback, and bounded visible image requests under virtualization.
- Test fresh light default, cached prepaint, persisted theme reconciliation, complete token coverage, and both-theme contrast/focus states.
- Test one shared audio controller with protocol URLs: one active track, play/pause, pointer and keyboard seek, page/filter/track transitions, ended/error recovery, and cleanup on unmount.
- Test keyboard navigation, language-menu semantics, icon tooltips/accessibility, visible focus, accessible names/live regions, contrast, and reduced-motion media queries.

### Manual functional matrix

- macOS Intel/Apple Silicon universal build: pick one file, drag one file, pick a nested folder, drag a folder, cancel mid-folder, run with each visible quick-setting combination, reopen app, open history detail, reveal run output, reveal a song, open a song with the system player, and delete history without deleting output.
- Inputs: supported encrypted audio, invalid encrypted-looking file, standard audio, companion files, duplicate flat-looking basenames in separate directories, output inside input, existing destination with overwrite off/on, unicode/long paths, and missing/renamed output after history capture.
- Verify directory details are visually flat while output remains mirrored.
- Verify source files remain untouched and no half-written final destination is visible after cancellation/failure.
- Verify metadata-off makes no provider request; metadata-on provider/tag failures remain warnings with playable decrypted output.
- Verify the header stays fixed during long home/detail scrolling; fresh state is light; theme/locale survive restart; the home ratio/history placement and compact detail remain usable at 920×640, 1024×700, and 1280×800.
- Use a run whose target song exists only on a later page to prove search/artist/album filters query the complete dataset. Change sort/filter while on a later page and confirm the UI resets to a valid first page with correct total/page labels.
- Play MP3 and FLAC fixtures, pause/resume, seek forward/back, switch rows, change page during playback, and leave detail. Confirm at most one track plays and no stale playback/object URL/listener remains.
- Rename/remove one recorded output and replace another path with a symlink; playback/reveal/system-open must fail safely with translated feedback while unaffected rows continue working.

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
- Existing Windows x86_64 CI/release packaging remains configured, but no new local/remote Windows build, installer, WebView2, runtime, or visual test is required for this cycle.
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
- **Dynamic filters and sorting can make pagination inconsistent or unsafe.** Keep sort columns/directions as Rust enums mapped to static SQL, bind all user text, return exact filtered totals, reset page state on query changes, and test matches that occur only beyond the former first cursor page.
- **Audio playback can accidentally broaden filesystem access or allocate whole songs.** Address media by persisted item ID, revalidate the exact destination on every request, content-probe an allowlisted type, implement bounded `HEAD`/range responses, expose only the custom media origin in CSP, and keep generic Tauri asset/filesystem access disabled.
- **Webview media behavior differs across WebKit and WebView2.** Validate MP3 and FLAC play/pause/seek, range requests, ended/error handling, page changes, and cleanup in both native webviews; treat unsupported container playback as a localized unavailable state rather than transcoding or weakening validation.
- **OS reveal/default-player integration is an intentional process boundary exception.** Keep it behind explicit user gestures and run/item-ID Rust commands, use a narrow platform adapter, and update architecture wording so no one mistakes it for permission to spawn the conversion CLI, a shell, or arbitrary programs.
- **Theme persistence can flash or become split-brain.** Treat Rust preferences as durable truth and localStorage only as a synchronous prepaint cache; default missing/invalid values to light, reconcile during bootstrap, and test first install, upgraded preferences, restart, and storage mismatch.
- **Removing overwrite from the UI changes repeat-run expectations.** Always send `overwrite = false`, keep existing destinations safe/skipped, retain old setting snapshots for historical truth, and state this behavior with concise result copy rather than an advanced pre-run toggle.

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
- [x] 2026-08-22: Reopened T5.4 after the real GitHub macOS release job failed immediately after Tauri bundling: the action produced the DMG but did not retain a build-tree `UnlockM.app`, so `find ... -name UnlockM.app` yielded empty and the silent `test -n` failed. This is consistent with prior local Tauri packaging behavior. The workflow must verify the finalized DMG by a read-only `$RUNNER_TEMP` mount rather than relying on an ephemeral bundle-tree app directory.
- [x] 2026-08-22: Reopened T5.4 after the real Actions publish job failed checksum validation. The macOS `.sha256` passed; Windows generated its manifest through PowerShell `Set-Content`, which wrote CRLF. Ubuntu `shasum -c` interpreted the trailing carriage return as part of the NSIS filename, reporting `No such file` despite the `.exe` being present. The fix must write an ASCII, BOM-free, LF-only Windows manifest and reject CR bytes explicitly before the shared checksum loop.
- [x] 2026-08-22: Recompleted T5.4 checksum publishing. Windows now constructs `hash␠␠filename␊` and writes it with `[IO.File]::WriteAllText(..., [Text.Encoding]::ASCII)`, which is BOM-free and LF-only. Before running `shasum -c`, publish rejects any CR byte with an artifact-specific message. The exact four-asset contract is unchanged. Local PowerShell is unavailable, so a cross-platform byte-level fixture wrote the same ASCII/LF manifest, proved no CR byte, and passed `shasum -a 256 -c`; workflow YAML/static policy, 42-file Vite+ format/lint/type, 35 frontend tests, build, license/security tests, and diff guard all pass. T5.6 remains open for a real Actions preview and Windows install/launch evidence.
- [x] 2026-08-22: Independent checksum-manifest re-review passed with no findings. The reviewer confirmed PowerShell path/encoding/newline compatibility, BOM-free `hash␠␠filename␊` bytes, Bash CR rejection under `set -e`, the unchanged checksum/exact-four-asset gates, and accurate D28/T5.6 state.
- [x] 2026-08-22: Recompleted T5.4 with final-DMG verification. The macOS workflow finds exactly one DMG, runs `hdiutil verify`, mounts it read-only/nobrowse in a unique `$RUNNER_TEMP` directory, and validates mounted `UnlockM.app` ad-hoc signature/resources/one executable/universal architectures. Its `trap` detaches/removes the mount on failure; the success path explicitly detaches/rmdirs and cancels the trap. The exact shell logic passed locally against the universal DMG (`Signature=adhoc`, `x86_64 arm64`, expected OFL/notices) with no remaining hdiutil/cargo/bundle process. Windows steps are unchanged. YAML parse, no-secret static policy, and diff guard also pass. T5.6 remains open for real Actions prerelease/Windows install evidence.
- [x] 2026-08-22: Independent T5.4 DMG-verifier re-review passed with no findings. The reviewer confirmed the exact-one-DMG/nullglob guard, final-image verification, read-only temporary mount, original-status-preserving failure cleanup, explicit successful detach/trap removal, macOS Bash compatibility, unchanged Windows branch, and accurate D27/T5.6 documentation.
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
- [x] 2026-08-22: Reopened this active plan for the user-supplied home/detail redesign instead of creating a duplicate plan. Read the sketch only as design evidence, inspected the implemented frontend, DTO/IPC, SQLite schema-v1 queries, current Tauri capability/CSP, core music snapshot path, and the locally installed Tauri 2.11.5 custom/asset protocol range implementation. Added M6–M9 and T6.1–T9.4; no production source, test, build, or configuration file changed in this Plan phase.
- [x] 2026-08-22: Planning checkpoint recorded that M6–M9 were not yet implemented. This historical state is superseded by the implementation/validation entries below; T9.3, T9.4, and T5.6 remain the active closure tasks.
- [x] 2026-08-22: Began M6 implementation. Per the user's latest validation scope, macOS is the only required complete native/runtime/visual platform for this cycle; Windows product code and existing CI/release definitions remain, but Windows testing is waived rather than left as a blocker. The redesign uses Phosphor rather than Lucide so all new interface icons come from one approved family.
- [x] 2026-08-22: Completed T6.1–T6.5. Preferences now deserialize missing themes as light; observer-only snapshots capture final output bytes/content format; schema-v2 migration adds nullable output facts and bounded legacy page repair; run summaries distinguish exact music count from companion-inclusive totals; SQL-backed detail queries apply search/facets/outcome/allowlisted sort before page slicing; the official opener sits behind ID-only Rust commands; and `unlockm-audio` serves only validated music-item `HEAD`/bounded ranges. Focused desktop Rust tests passed before the full workspace gate.
- [x] 2026-08-22: Completed T7.1–T8.6. The fixed bilingual header, light-default/dark themes, Phosphor actions, 1.6:1 composer, concise safe settings, blue primary action, full-width history rows, compact detail facts, true 25-song pages, search/artist/album/result/sort controls, actual FLAC/MP3 size/type facts, one shared player, seek controls, Finder reveal, and default-player open actions are implemented. `rtk vp -C desktop check`, 33 frontend tests across 12 suites, production build, and Tauri debug no-bundle/app bundle builds passed.
- [x] 2026-08-22: BrowserOS visual QA passed at a 1527×824 viewport in light/dark and at the narrow supported layout: desktop composer columns measured about 733.5/458.5 px, history begins below the composer, `scrollWidth <= innerWidth`, narrow layout collapses to one column, and the sticky header remained at top 0 after scrolling. The native macOS WebKit app displayed the same light/dark home and compact detail hierarchy with no Diagnostics banner.
- [x] 2026-08-22: Native macOS functional QA passed against the persisted 80-song/246-companion run and a new single-file conversion. The old schema history lazily exposed four 25-song pages with artwork, album values, content-probed FLAC and real sizes; search reduced the complete run to one “Frankenstein” result. In-app playback advanced from 0 to 24.7 seconds, seeking jumped to 2:57, and pause returned the row to “播放”. Finder selected the exact converted FLAC, while the system-open action launched IINA with the same 3:22 track. Dark theme, Chinese locale, and history survived close/relaunch.
- [x] 2026-08-22: New conversion smoke selected `samples/Claire Rosinkranz - Frankenstein.ncm`, wrote `/private/tmp/unlockm-redesign-qa.82xPTM/Claire Rosinkranz - Frankenstein.flac`, and persisted a new one-song history/detail row with FLAC and 22.5 MiB. `file` reported 16-bit stereo 44.1 kHz with 8,919,168 samples; output size is 23,548,419 bytes; the source SHA-256 remains `098f5f4c8a869b27cfd8c50efe54a914aa3652664848bc8f0b6d47feb8020b25`. Temporarily moving the QA output produced translated non-destructive playback failure feedback; the file was restored.
- [x] 2026-08-22: Completed T9.1–T9.2 implementation/documentation. Security policy retains the exact dialog/event capability allowlist, rejects generic asset/opener/shell/filesystem access, and asserts only the custom media origins. Architecture, both READMEs, notices, and license audit now describe theme/query/playback/native-action boundaries plus Phosphor 2.1.10 and opener 2.5.4. Full strict Rust validation passed at this checkpoint: workspace Clippy clean and 171 tests across 18 suites; a later final gate will include the added 1,001-item regression.
- [x] 2026-08-22: Pre-review gates passed after the 1,001-item regression and typed playback error update: Rust formatting, strict workspace Clippy, and 172 tests across 18 suites; Vite+ 45-file format check, lint/type check, 33 tests across 12 suites, production build; and `rtk git diff --check`. Status moved to `In Review`; T9.3 final package evidence and T9.4 review closure remain open.
- [x] 2026-08-22 review-plan first pass (`Needs Changes`, resolved): Major - normal player cleanup no longer calls `audio.load()` after source removal, and a regression proves unmount adds no reload beyond the intentional track load. Minor - the ARIA menu now has selected-item focus, ArrowUp/ArrowDown/Home/End roving focus, Escape focus restoration, tests, and semantic `app.localName` lookup. Plan - `Current Context` and completion evidence now describe the implemented state and remaining closure only. Vite+ format/check and 35 tests passed before re-review.
- [x] 2026-08-22 review-plan re-review (`Pass`): no blocker, major, or meaningful minor findings remain. The implementation matches M6–M9 scope and security/architecture decisions; task checkboxes and validation claims are accurate. Required code/plan updates: none. Remaining gaps are intentionally limited to T9.3 final macOS validation/package evidence and T5.6 universal DMG closure, so status returns to `In Progress` for those gates.
- [x] 2026-08-22: Completed T9.3 and T5.6. Final gates are Rust formatter, strict workspace Clippy, 172 tests across 18 suites; Vite+ 45-file format check, lint/type check, 35 tests across 12 suites, and production build; Tauri debug no-bundle/app build; BrowserOS responsive light/dark QA; native macOS conversion/restart/history/search/page/play/pause/seek/Finder/IINA/missing-output QA; and `rtk git diff --check`. WebKit memory logging during the 80-song paged detail remained 47–53 MiB, and the UI rendered at most one 25-song page.
- [x] 2026-08-22: Pre-M10 universal package evidence (superseded) recorded a 15,855,746-byte DMG with SHA-256 `14c4ab58283f17ed3e9eaaadbbeb12593e0802110bfb2d77dd74e2634c2d0c4b`, VALID checksum, dual architectures, ad-hoc signature, resources, and no sidecar under the erroneous `world.fansion.unlockm` identifier. M10 replaces this artifact and identifier; it is retained only as chronological evidence.
- [x] 2026-08-22 completion audit: every Task Breakdown item is checked; M6–M9 acceptance and macOS validation evidence are present; first-pass review findings are resolved and re-review passed; documentation matches the implementation; source/output safety and no-sidecar invariants passed existing/full regressions. Windows runtime/installer/visual testing is not claimed and is explicitly waived by user direction for this cycle. Status moved to `Done`.
- [x] 2026-08-22: Reopened the completed plan for four explicit follow-ups: canonical `world.fansionia.unlockm` identity with history continuity, corrected detail summary fit, native About credits/copyright, and severe metadata-off artist loss. Added M10/T10.1–T10.5 before implementation; previous M6–M9 evidence remains valid but the old DMG hash is superseded once M10 ships.
- [x] 2026-08-22: Completed T10.1. Tauri configuration and current docs use `world.fansionia.unlockm`; schema-safe SQLite online backup initializes only an absent new database from the legacy identifier and preserves the old copy. First-run/idempotence tests passed, and native launch under the new identifier opened a migrated 4-run/83-song database at `~/Library/Application Support/world.fansionia.unlockm/history.sqlite`.
- [x] 2026-08-22: Completed T10.2 native QA. At the 1080×760 macOS window, `Claire Rosinkranz - Frankenstein.ncm`, the full wrapped source path, output path, all four facts, and `2026年8月22日 14:37` rendered without ellipsis; the metric gap is visibly tighter and the card remains compact.
- [x] 2026-08-22: Completed T10.3 native QA through a uniquely named temporary copy of the exact corrected app binary (only its QA display name differed to bypass stale LaunchServices cache). The standard File/Edit/View/Window/Help menus remained. About displayed the requested Chinese credit, English Additive Cookie/Moxianbao/vincentlee73 lines, and `Copyright © Hamlet Fansion (Peilin Fan)` with the UnlockM icon/version.
- [x] 2026-08-22: Completed T10.4. Root cause was twofold: output-tag reads replaced filename fallback with `None`, and real NCM `artist` uses nested arrays such as `[[name,id]]`, which the parser ignored. Final-tag snapshots now retain fallbacks, NCM nested arrays are parsed, trusted container fields merge into observer-only snapshots even when metadata update is off, and bounded detail reads repair existing null artist/title rows. The real repository sample and native detail now show `Claire Rosinkranz`; focused core, parser, desktop repair, no-audio-mutation, About, and identifier-backup tests pass.
- [x] 2026-08-22 M10 review-plan first pass (`Needs Changes`, resolved): Minor - custom About metadata now exposes the bilingual text through both macOS `credits` and non-macOS `comments`, with standard File/Edit/Window/Help actions retained. Minor - a failed final migration rename now removes the owned temporary backup. Focused tests and strict desktop Clippy passed.
- [x] 2026-08-22 M10 re-review (`Pass`): all four user-visible requirements and both first-pass review findings are implemented with accurate plan state. No blocker, major, or meaningful minor remains. T10.5 now requires only final full gates, corrected universal DMG identity/hash inspection, and completion audit.
- [x] 2026-08-22: Completed T10.5 final gates: Rust formatting, strict workspace Clippy, and 178 tests across 18 suites; Vite+ 45-file format check, lint/type check, 35 tests across 12 suites, production build; `rtk git diff --check`; native macOS M10 screenshots/About interaction; and universal Tauri packaging. Windows execution remains explicitly waived.
- [x] 2026-08-22: Corrected final artifact `target/universal-apple-darwin/release/bundle/dmg/UnlockM_0.1.0_universal.dmg` is 15,870,044 bytes with SHA-256 `0a8ed33d4f454799f4fdf8b61f3f097651638c972060fa59b40717ffdc26335f`; `hdiutil verify` returned VALID. Read-only inspection proved `world.fansionia.unlockm` in Info.plist/codesign, `Copyright © Hamlet Fansion (Peilin Fan)`, `x86_64 arm64`, `Signature=adhoc`, complete license/icon resources, requested About strings in the binary, and exactly one sidecar-free executable. The final universal app replaced the temporary debug installation at `/Applications/UnlockM.app` and LaunchServices was refreshed.
- [x] 2026-08-22 M10 completion audit: all T10 tasks and review findings are closed. Native evidence shows the long title/full paths/full completion timestamp without clipping, About displays all requested Chinese/English names and copyright, migrated history is present under `world.fansionia.unlockm`, and existing/new Claire rows display `Claire Rosinkranz`. The real sample parser/no-write tests prove metadata-off artist correctness. Status returned to `Done`.
- [x] 2026-08-22: Reopened the completed plan for M11 at the user's request. The repository was clean at `c659fb9`, `main` was one commit ahead of both `origin/main` and `github/main`, `v0.2.0` was unused locally, and the existing tag workflow requires matching Tauri/Cargo versions before producing the four unsigned preview assets. Japanese localization, the `0.2.0` version bump, validation/review, commit/tag, GitHub push, and live prerelease evidence remain in progress.
- [x] 2026-08-22: Completed T11.1–T11.3 implementation. Added complete Japanese semantic-key parity, `ja`/`ja-JP` detection, three-item roving-focus language selection and persisted Japanese product title; extended Rust/TypeScript bootstrap fixtures to three locales; and updated root Cargo, desktop Cargo, frontend package, Tauri config, and both workspace lock entries to `0.2.0`. Focused Vite+ locale/App tests passed (10 tests), the Rust bootstrap contract test passed, `cargo metadata --locked` reports both workspace packages at 0.2.0, and the local release version guard accepts `v0.2.0`.
- [x] 2026-08-22: M11 pre-review gates passed: `cargo fmt --check`, strict locked workspace Clippy, 178 Rust tests across 18 suites, frozen Vite+ install, 46-file frontend format/lint/type check, 36 frontend tests across 12 suites, production build, and `git diff --check`. A local ad-hoc-signed universal package produced `UnlockM_0.2.0_universal.dmg`; `hdiutil verify` returned VALID, and read-only inspection confirmed version `0.2.0`, identifier `world.fansionia.unlockm`, `x86_64 arm64`, `Signature=adhoc`, and exactly one executable. The 15,873,310-byte candidate SHA-256 is `a198a9a7c779c9caea9a5e17f86eae3346e3e3d7753dc8efd824a495bc4e94d9`. Status moved to `In Review`; commit/tag/push/live Actions evidence remains pending.
- [x] 2026-08-22 M11 review-plan first pass (`Pass with Notes`, resolved): Minor - the frontend header and Rust bootstrap construction repeated the three-locale allowlist, creating an avoidable drift risk. The header now reuses exported `supportedLocales`, and Rust uses one `SUPPORTED_LOCALES` constant. Affected Vite+ format/check and 10 locale/App tests plus desktop Rust format/strict Clippy/bootstrap contract tests passed.
- [x] 2026-08-22 M11 re-review (`Pass`): no blocker, major, or meaningful minor findings remain. Scope, locale/key/interpolation behavior, keyboard selection/persistence, IPC fixtures, version metadata, security boundary, local artifact evidence, and plan state are accurate. T11.4 is complete; status returns to `In Progress` solely for commit/tag/push and live GitHub prerelease evidence under T11.5.
- [x] 2026-08-22 first GitHub v0.2.0 run (`32564012836`, superseded): `main` and annotated tag `v0.2.0` were atomically pushed at `957a9d7`, but the macOS package job failed in `Run complete Rust workspace tests` with exit 101 before packaging; publish did not run. BrowserOS inspection and source audit traced the clean-checkout-only failure to three M10 regressions reading an unversioned sibling path under `../samples`, which exists locally but not in GitHub Actions.
- [x] 2026-08-22 release-blocker fix and re-review: the NCM parser, metadata-off snapshot, and bounded history-repair regressions now synthesize inputs from the committed deterministic `tests/support/ncm_fixture.rs` plus the committed silent MP3 fixture. Exact nested-array parsing remains asserted separately, propagation expectations derive from parsed container evidence, and audio immutability remains asserted. No production path changed. `rg` finds no remaining sibling-sample dependency; formatter, strict locked workspace Clippy, and all 178 Rust tests across 18 suites pass. Re-review status: `Pass`; the failed tag/run must be replaced with an exact new tag at the fix commit before republishing.

## Discoveries

- **2026-08-22 clean-checkout release discovery:** Three otherwise-correct M10 regressions used a 24 MB real NCM sample from the checkout's unversioned sibling `../samples` repository. Local full gates therefore passed while GitHub's clean checkout failed at the complete Rust test step. Release-critical tests must use committed deterministic fixtures; real sibling samples remain manual/native QA evidence only.
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
- **2026-08-22 Actions DMG discovery:** Tauri's macOS DMG bundler can clean the intermediate `.app` from `target/.../bundle` after producing the DMG. The final disk image—not the transient build-tree application—is the authoritative artifact for signature/resource/architecture inspection.
- **2026-08-22 cross-platform checksum discovery:** PowerShell `Set-Content` is not a portable SHA-256 manifest writer for an Ubuntu `shasum -c` consumer because its default line ending is CRLF. Write the exact `hash␠␠filename␊` bytes through `System.IO.File.WriteAllText` with `Text.Encoding.ASCII`, and fail publish validation early if any CR byte exists.
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
- **2026-08-22 redesign discovery:** The current top-level `app-grid` makes Composer and history peers, while the requested hierarchy needs a composer-internal golden-ratio split followed by history at full width. This is a structural `App.tsx`/Composer/CSS change, not a token-only restyle.
- **2026-08-22 query discovery:** Current `get_run_items` keysets by ordinal and `HistoryDetail` filters only the already loaded array. True search/facet/sort pagination therefore belongs in Rust/SQLite and must return exact filtered totals; a client-only filter would silently miss later-page matches.
- **2026-08-22 output-fact discovery:** `logical_bytes` is sourced from the input during runner admission and cannot be labeled as converted file size. The observer-enabled final snapshot must capture actual destination length and content-probed output format, with nullable schema-v2 fields for migration safety.
- **2026-08-22 playback discovery:** Tauri 2.11.5 supports asynchronous custom protocols, and its bundled asset protocol demonstrates `HEAD`, `Accept-Ranges`, `206`, `416`, MIME probing, and bounded range chunks. A dedicated history-item protocol can reuse those semantics without granting the generic asset protocol or accepting raw paths.
- **2026-08-22 native-action discovery:** “In-app playback”, “reveal in Finder/Explorer”, and “open with the system player” are three separate actions. The first uses one shared controlled audio element; the latter two are explicit OS side effects behind separate item-ID commands and must not be inferred as in-app playback state.
- **2026-08-22 preference discovery:** The current preference JSON has locale and quick settings only. Adding theme requires backward-compatible deserialization; a localStorage value can prevent first-paint flash but cannot replace Rust-owned persistence.
- **2026-08-22 custom-URL discovery:** Tauri `convertFileSrc` percent-encodes embedded slashes, so the frontend passes the bare numeric item ID and the protocol accepts `/ID` (while tests also accept `/item/ID`). This preserves the ID-only boundary and avoids accidentally parsing an encoded path segment.
- **2026-08-22 native playback evidence:** macOS WebKit issued the custom-scheme range requests successfully for persisted FLAC files; playback, time updates, seeking, pause, page/query transitions, and missing-output `FormatError` all stayed inside the product error boundary. Unified logs contained no panic/abort/crash evidence during QA.
- **2026-08-22 opener evidence:** The exact-file reveal action selected `Claire Rosinkranz - Frankenstein.flac` in Finder, and system open launched the user's associated IINA player with the same 3:22 track. These are distinct from the in-app audio controller, as planned.

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
- **2026-08-22 — D27:** Verify the macOS preview app only after read-only mounting the final DMG at a unique `$RUNNER_TEMP` directory. A shell trap must detach the image and remove the mount directory even after validation failure; the success path explicitly performs the same cleanup and disables the trap. Do not trust the post-bundle build-tree `.app` path.
- **2026-08-22 — D28:** Treat cross-platform checksum manifests as byte-level release artifacts: generate Windows manifests as BOM-free ASCII with one LF terminator, assert no carriage returns before `shasum -c`, and retain the exact four-asset contract.
- **2026-08-22 — D29:** Supersede D4's UI exposure of overwrite. The redesigned primary interface exposes metadata refresh and directory-only companion copy, always sends `overwrite = false`, and leaves output layout/performance/key settings at core defaults. Existing run snapshots and backend compatibility fields remain intact.
- **2026-08-22 — D30:** Supersede D8's dark-only visual direction with a complete manually toggled light/dark pair whose fresh/missing-preference default is light. Use `UnlockM · 音乐解锁` and `UnlockM · Music Unlocker` as localized title-bar labels while retaining `UnlockM` as the product name.
- **2026-08-22 — D31:** Treat the supplied sketch as a layout reference: fixed shared header; roughly `1.6:1` drop/action composer; output control first, Convert last; full-width history below; compact detail summary; flat song rows. Do not copy sketch annotations as runtime instructions or add unspecified product behavior.
- **2026-08-22 — D32:** Replace detail cursor accumulation/client-only filtering with Rust-owned SQL query pagination. Search, artist/album/outcome filters, sort, exact counts, and page size apply before pagination; run history itself retains newest-first keyset pagination.
- **2026-08-22 — D33:** Persist actual published output length and content-probed container label as nullable schema-v2 item facts. Never relabel input `logical_bytes` as output size; repair legacy rows only on bounded page reads when their destinations still validate.
- **2026-08-22 — D34:** Implement in-app playback through one shared frontend audio controller and a Rust `unlockm-audio` custom protocol keyed only by history item ID. Support bounded byte ranges and narrow `media-src`; do not enable Tauri's generic asset protocol, return audio through SQLite/IPC, or add transcoding.
- **2026-08-22 — D35:** Permit explicit run-output reveal, exact-item reveal, and default-system-player open as narrow user-triggered desktop actions. Rust commands accept only persisted IDs and revalidate targets; no generic opener permission, shell command surface, or conversion subprocess is introduced. This is the sole documented OS-open exception to the existing process boundary.
- **2026-08-22 — D36:** Complete this redesign through full macOS validation only, as explicitly requested. Preserve Windows support code, CI, and release definitions without claiming new Windows runtime evidence. Use one Phosphor icon family for all new interface actions; this supersedes the planning-time Lucide choice.
- **2026-08-22 — D37:** Correct the canonical identifier to `world.fansionia.unlockm`. Preserve legacy history by using SQLite's online backup API only when the new database is absent; retain the legacy database as a recoverable copy and never merge/overwrite a non-empty new database.
- **2026-08-22 — D38:** History display metadata may use trusted local NCM container evidence independently of `update_metadata`; this does not authorize network access or audio-tag mutation. Text uses container evidence over final tags when present, while embedded output artwork remains preferred over container artwork.
- **2026-08-22 — D39:** Keep the native standard menu structure and customize only About metadata. Credits are bilingual in one panel so they remain correct regardless of current webview locale.
- **2026-08-22 — D40:** Ship Japanese as the third complete locale under `ja-JP`. Japanese platform locales select it on first launch, unsupported locales continue to fall back to English, and the persisted preference/bootstrap contract remains the authoritative explicit selection. Publish this increment as the existing checksum-backed unsigned-preview channel at `v0.2.0`.
- **2026-08-22 — D41:** Release-blocking automated tests must be hermetic within this repository. Use the committed deterministic NCM/header/audio fixtures for parser/snapshot/history regressions and retain the external 24 MB Claire sample only as explicit native/manual QA evidence; never infer clean-checkout coverage from its local presence.

## Completion Evidence

### Baseline evidence before the 2026-08-22 redesign

The pre-redesign desktop implementation/configuration, documentation, no-secret unsigned-preview CI/release definitions, licensing resources, and baseline local validation were complete. The Progress log records the locked 162-test/18-suite workspace evidence, 35 frontend tests, policy/license contracts, Vite+-only workflow guards, BrowserOS visual QA, native macOS debug-app conversion/restart/history QA, and read-only unsigned arm64 plus universal DMG content/checksum smokes. The final baseline local universal app verified as `Signature=adhoc`, dual-architecture, resource-complete, and sidecar-free; this is intentionally not a notarized/trusted production signature.

This baseline evidence alone does not validate M6–M9. The current Progress entries now add implementation gates, BrowserOS visual results, macOS WebKit playback/seek, Finder/IINA actions, restart persistence, schema-v1 repair, and a new conversion smoke. Final review/package evidence is still recorded separately below.

### Redesign completion evidence

M6–M10 and T5.6 are complete. The previous `world.fansion.unlockm` DMG remains historical redesign evidence only and is superseded for release purposes by the corrected M10 artifact below.

M11 is in progress for complete `ja-JP` localization, consistent `0.2.0` package metadata, validated commit/tag creation, and the real GitHub `v0.2.0` prerelease. The authoritative v0.2.0 remote workflow and artifact evidence will be recorded after publication; no successful release is claimed before then.

### Final macOS preview evidence

Windows runtime/installer evidence is waived for this cycle by explicit user direction and is not a completion gap. Final macOS evidence includes:

- task/milestone completion dates and commit hashes,
- exact Rust/frontend/native packaging commands and outcomes,
- macOS artifact name, architectures, size, SHA-256 checksum, and bundle-content inspection,
- macOS ad-hoc verification output,
- manual QA results for file/folder picker and drag/drop, visible quick settings, cancellation, partial failure, restart persistence, light/dark and both locales, fixed header/home/history layout, detail metadata/artwork/search/filter/sort/pages, playback/seek, run/item reveal, system-player open, and reduced motion,
- confirmation that source files were retained, outputs were atomically published, metadata-off made no provider request, and no `unlockm` CLI sidecar/process was present,
- review findings and their resolution or explicit acceptance.

The authoritative final artifact is `UnlockM_0.1.0_universal.dmg`, 15,870,044 bytes, SHA-256 `0a8ed33d4f454799f4fdf8b61f3f097651638c972060fa59b40717ffdc26335f`, verified VALID, ad-hoc signed, universal, resource-complete, and sidecar-free. Its identifier is `world.fansionia.unlockm`; the native About metadata, full detail layout, legacy history backup, and metadata-off artist fix are included.
