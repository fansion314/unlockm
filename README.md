# unlockm

[简体中文](README.zh.md)

`unlockm` turns supported encrypted music downloads into normal, playable
audio files. It decrypts the original audio stream without transcoding, keeps
your source files, and can preserve the input directory layout in a separate
output folder.

## Why unlockm?

- **No quality loss:** audio is decrypted, not re-encoded.
- **Safe by default:** source files are never deleted, and completed outputs
  are published atomically.
- **Useful for whole libraries:** directories are scanned recursively, while
  cover images, lyrics, and other companion files are copied by default.
- **Clear feedback:** an interactive terminal shows overall progress and the
  file currently being processed.
- **Self-contained crypto:** all format-specific decryption code used by the
  program lives in this repository; there is no runtime dependency on an
  external crypto service or FFmpeg.

## UnlockM desktop app (macOS and Windows)

`UnlockM` is the native desktop host for the same Rust engine. It is currently
supported on macOS and Windows; the command-line `unlockm` program remains the
cross-platform option, including Linux and watch mode.

The first desktop release deliberately stays focused:

- choose or drop exactly one file or directory, then optionally choose an
  output directory;
- adjust metadata refresh, companion-file copy, and atomic overwrite before
  converting;
- follow one active conversion, cancel it safely, and reopen its persisted
  history/detail view after restarting the app;
- use English or Simplified Chinese; inspect captured metadata, outcomes, and
  lazy album artwork in a virtualized detail list.

Desktop watch mode, playlists, playback, cloud sync, accounts, telemetry, and
manual key entry are not part of v1. Conversion is in-process: the app links
this Rust library directly and does **not** launch the `unlockm` CLI, a
sidecar, shell command, or daemon.

### Desktop preview installers and local data

Every `v*` tag automatically publishes an **unsigned preview prerelease**: one
universal macOS DMG and one Windows x86_64 NSIS installer, each with a SHA-256
file. The workflow needs no GitHub environment, certificate, password, or
signing secret. GitHub-hosted Actions minutes and artifact storage still count
against the repository owner's GitHub plan and quota, so private-repository
overages may be billable. The macOS app receives only Tauri's ad-hoc signature;
it is not notarized. The Windows app and installer are intentionally
Authenticode `NotSigned`. These are installers rather than portable command-line
binaries.

Before first opening a preview, download it only from the matching GitHub
prerelease and verify its attached checksum:

```sh
# macOS
shasum -a 256 UnlockM-*-macos-universal-unsigned.dmg

# Windows PowerShell
Get-FileHash .\UnlockM-*-windows-x64-unsigned-setup.exe -Algorithm SHA256
```

Compare the result with the corresponding `.sha256` release asset. On macOS,
if Gatekeeper asks after verification, use Finder's per-app **Control-click →
Open** flow and confirm Open. On Windows, if SmartScreen identifies the
verified preview as unknown, use its per-app **More info → Run anyway** choice
only when you trust the release and matching hash. Do not disable Gatekeeper,
SmartScreen, or other system-wide protections. A signed/notarized channel may
be added separately in the future.

UnlockM stores its local `history.sqlite` database in Tauri's application-data
directory (normally `~/Library/Application Support/world.fansion.unlockm/` on
macOS and `%APPDATA%/world.fansion.unlockm/` on Windows). It contains local
conversion records, absolute source/output paths, settings snapshots, outcome
details, captured metadata, and deduplicated album artwork (up to 10 MiB per
image). It never stores audio payloads, sends telemetry, or deletes recorded
music files. The app's clear/remove controls delete database records only.

### Desktop development

Use Vite+ as the only frontend lifecycle entry point. The project locks its
pnpm backend through `desktop/pnpm-lock.yaml`; do not run npm, pnpm, yarn, or
bun directly.

```sh
vp -C desktop install --frozen-lockfile
vp -C desktop check
vp -C desktop test
vp -C desktop build
vp -C desktop exec tauri dev
```

For a local native compilation without creating an installer:

```sh
vp -C desktop exec tauri build --debug --no-bundle
```

Preview packaging is performed automatically by the tag workflow and has no
release secrets. It is intentionally distinct from a future optional
signed/notarized distribution channel.

The application bundle also includes [third-party notices](THIRD_PARTY_NOTICES.md)
and the complete [SIL Open Font License 1.1](third_party/licenses/SIL-OFL-1.1.txt)
for its locally bundled Bricolage Grotesque and Manrope fonts.

## Install

Rust 1.89 or newer is required. The lockfile is part of the release contract.

```sh
cd unlockm
cargo install --path . --locked
unlockm --version
```

To build without installing:

```sh
cargo build --release --locked
./target/release/unlockm --help
```

## Quick start

Convert a music directory and mirror its layout under a new output directory:

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked
```

Convert it and conservatively improve tags and cover art:

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --update-metadata
```

Convert encrypted audio only, without copying lyrics, images, or other
ordinary files:

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --no-copy
```

Run `unlockm --help` for every option and `unlockm --supported-ext` for the
complete, versioned suffix list.

## Where files go

The default **mirror** layout keeps subdirectories:

```text
Encrypted/Artist/Album/song.ncm
                    ↓
Unlocked/Artist/Album/song.flac
```

Ordinary companion files are copied unless `--no-copy` is set. Existing
destinations are skipped unless you explicitly pass `--overwrite`; replacement
still happens only after the new file is complete. If the output directory is
inside the input tree, unlockm ignores it during scanning and watch mode.

Use `--flat` to place every result directly in the output root:

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --flat
```

If two inputs would produce the same flat filename, both are reported as a
conflict. unlockm never picks an arbitrary winner.

## Progress and command output

Interactive terminals show two live rows:

```text
[========--------------] 118/326  36.2%  2.4s
Decrypting: Artist/Album/song.ncm (+1 active)
```

The count advances as each file finishes. The second row shows the latest
active relative path; `(+1 active)` means one other file is being processed.

`--progress auto` is the default and animates only on a terminal. Use
`--progress always` to force it or `--progress never` to disable it. Redirected
human output contains no terminal control codes and ends with a stable summary:

```text
unlockm: converted=80, copied=246, skipped=0, conflicted=0, failed=0
```

For scripts, `--json-logs` disables live progress and emits JSON diagnostics
plus a final JSON summary. Progress and diagnostics are written to stderr.

## Concurrency and performance

`--jobs` is the maximum number of **files** processed at the same time. The
default is two file jobs; it does not limit a run to two files, and it is not a
promise that the program uses only two operating-system threads.

Two was the fastest default on the representative 3.4 GiB mixed APFS workload
with metadata updates: median times were 3.62s at one job, 2.99s at two, 3.39s
at four, and 5.66s at eight. More simultaneous decrypt, copy, and tag writes
can create storage contention, so more jobs are not always faster. You can tune
the value for a different drive or workload:

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --jobs 4
```

`--metadata-jobs` is separate: it limits concurrent metadata network requests,
not file conversion jobs.

## Metadata and privacy

Without `--update-metadata`, unlockm makes no metadata network requests and
retains embedded tags and artwork. With the flag enabled, it can enrich NCM
container metadata and verified QMC identities. Existing information wins when
a remote result is absent, ambiguous, invalid, or unavailable.

Metadata lookup and tag-writing failures are warnings rather than conversion
failures. Artwork is checked for size, file signature, and decodability before
use. Requests are HTTPS-only and restricted to the documented QQ/Netease API
and artwork hosts; local addresses, IP literals, userinfo, and unexpected hosts
are rejected before dispatch.

## Supported format families and keys

| Family | Notes |
| --- | --- |
| NCM | NetEase Cloud Music containers, including embedded metadata and artwork. |
| QMC / QQ / Moo | QMC v1/v2 and aliases. QMC v2 tries its embedded footer key first, followed by configured key sources. |
| KGM / KGG / KGMA / VPR | KuGou formats. KGM v5 may require a KuGou encrypted key database. |
| KWM | Kuwo Music formats. |
| Xiami | Xiami encrypted audio. |
| Ximalaya | Android and PC X2M/X3M variants. |
| TM | QQ Music iOS TM variants. |

QMC v2 key options are tried after an embedded footer key:

1. `--qmc-key-map` — a JSON object or `identifier=ekey` lines.
2. `--qmc-mmkv` — a QQMusic MMKV store; use `--qmc-mmkv-key` for an
   encrypted store. Its matching `.crc` file must be present.
3. `--qmc-ekey` — one explicit fallback ekey.

On macOS, unlockm also discovers conventional QQMusic 8.x MMKV and 10.x
plist-derived stores when available. KGM v5 accepts `--kgg-db` and can discover
the conventional Windows database location. Secrets are used only to build
in-memory key maps and are never logged.

A registered encrypted suffix must also pass format validation. Corrupt or
keyless encrypted inputs fail per file instead of being copied as ciphertext.

## Watch mode

`--watch` first completes the initial directory conversion, then recursively
watches for stable file changes:

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --watch --overwrite
```

Create, modify, rename, and remove event bursts are coalesced. If a file changes
while it is running, unlockm schedules one clean follow-up pass. Ctrl-C or a
remove event cancels private temporary work without deleting the source or
publishing a partial result.

## Exit status and troubleshooting

- Exit `0`: every file completed, copied, or was safely skipped.
- Exit `1`: the command or overall run could not start or continue.
- Exit `3`: some files failed or conflicted; unaffected files were still
  processed.

Common cases:

- **Everything is skipped:** the destinations already exist; use `--overwrite`
  only if replacing them is intended.
- **An encrypted file fails validation:** confirm that it is not corrupt and
  that the required QMC/KGM key source is available.
- **Metadata warnings appear:** the decrypted output is still kept; metadata
  enrichment is best effort.
- **No animated bar appears in redirected output:** this is intentional; use
  `--progress always` only when terminal-style output is desired.
- **A faster drive performs differently:** benchmark `--jobs 1`, `2`, and `4`
  on a representative copy of the library before changing the default.

## Safety model

Decryption and copies are streamed with bounded buffers. Destination collisions
are rejected before heavy work, output paths remain contained, and a destination
becomes visible only after its private same-directory temporary file is
complete. Source size and modification time are checked again before
publication, and metadata recovery never replaces valid decrypted audio with a
partially modified file.

## License and notices

New project code is MIT licensed. See [LICENSE](LICENSE) and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for copied-code and fixture
attribution.
