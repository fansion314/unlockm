# unlockm

`unlockm` converts supported encrypted music downloads to their native playable
audio without transcoding. It retains sources, keeps memory bounded by
`--jobs * --buffer-size`, and only exposes a destination after an atomic publish.

## Build and install

Rust 1.75+ is required. The lockfile is part of the release contract.

```sh
cargo build --release --locked
./target/release/unlockm --help
```

## Examples

```sh
# Mirror a tree into a separate output directory (the default layout).
unlockm -o ./unlocked ./downloads

# Flatten files; same-run name collisions are errors, never arbitrary winners.
unlockm --flat --no-copy -o ./music ./downloads

# Convert the initial tree, then recursively monitor later changes.
unlockm --watch --overwrite -o ./unlocked ./downloads
```

Directory inputs are recursive, mirror empty directories, and copy ordinary
companion files by default. `--no-copy` affects only passthrough files; it never
suppresses encrypted-audio conversion. Existing destinations are skipped unless
`--overwrite` is given. Overwrite first completes a same-directory temporary and
retains a recoverable old destination until publication succeeds. An output root
inside the input tree and unlockm-owned temporary files are ignored by scan and
watch processing. In-place passthrough is a no-op.

`--watch` requires a directory. It subscribes recursively after the initial
batch scan, normalizes create/modify/rename/remove events, waits for matching
size/mtime observations, and coalesces storms. A change while a path is running
causes exactly one follow-up pass. Ctrl-C or a remove cancels owned temporary
work; it never deletes a source or publishes a partial output. A low-frequency
reconciliation protects against notification loss.

## Formats and keys

Run `unlockm --supported-ext` for the complete, versioned suffix list. The
supported families are NCM; QMC v1/v2 and their QQ/Moo aliases; KGM/KGG/KGMA/VPR;
KWM; Xiami; Ximalaya; and QQ iOS TM. Standard audio that merely has a registered
suffix is passed through only after positive probing. Corrupt or keyless
registered inputs fail per-file instead of being reported as copied ciphertext.

QMC v2 prefers its embedded footer ekey, then `--qmc-key-map` (JSON object or
`identifier=ekey` lines), `--qmc-mmkv`, and `--qmc-ekey`. `--qmc-mmkv-key`
accepts an MMKV password for an encrypted store; the matching `<store>.crc`
must be present because it supplies the MMKV IV. The password is only used to
load the in-memory map and is never logged. On macOS, QQMusic 8.x MMKV and
10.x plist-derived encrypted stores are discovered automatically when present.
KGM v5 uses `--kgg-db`, or its conventional Windows database when it can be
found.

## Metadata, privacy, and diagnostics

Without `--update-metadata`, conversion makes no metadata network request and
retains embedded tags/artwork. With it, NCM container data and QMC footer
identities may enrich a staged output. QMC uses an exact resource/media identity
request when available; only files without that identity may use a strict unique
title/artist/duration search. Filename search is only a hint: ambiguous or
offline results leave existing tags untouched. Artwork precedence is embedded
audio art, container art, exact lookup art, then verified-search art. Artwork is
size, magic, and decode validated. Network/provider/tag failures are warnings,
never conversion failures; there is no FFmpeg runtime dependency.

Metadata requests use HTTPS only. Provider RPC is pinned to QQ's API host, and
remote artwork is accepted only from the documented QQ and Netease music CDNs;
URLs with local hosts, IP literals, userinfo, or other origins are rejected
before a request is dispatched.

The default log level is `warn`; use `--log-level debug` for per-file diagnosis
or `--json-logs` for structured output. A batch with conflicts or file failures
exits 3 after processing unaffected siblings. Check `--help` for all budgets and
paths. If a key is absent, confirm its explicit source instead of attempting a
raw key fallback.

## License and notices

New project code is MIT licensed. See [LICENSE](LICENSE) and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for the pinned crypto reference,
fixture attribution, and release-audit requirements.
