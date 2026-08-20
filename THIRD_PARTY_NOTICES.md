# Third-party notices

`unlockm` uses the author's `lib_um_crypto_rust` packages at revision
`5dad838c33c3e4aab198d995160d3fbb9968470f`. Their individual crate manifests
and repository license determine their terms.

The MMKV reader and macOS QQMusic discovery logic in `src/keys/qmc.rs` are a
small Rust port of `github.com/unlock-music/go-mmkv` v0.1.4 and the Go
reference's `cli/internal/mmkv` / `cli/algo/qmc/qmmac` code. Those sources are
MIT licensed; the port retains their AES-128-CFB, CRC-IV, container, and macOS
derivation behavior.

The deterministic QMC footer and NCM parser vectors in `src/formats.rs` are
textual copies of `um_crypto/qmc/src/footer/fixtures/ekey_pc_enc_v1.bin` and
`um_crypto/ncm/src/__fixture__/ncm_test1.bin` at that revision. They are
provided by Project Unlock Music under MIT and Apache-2.0; no local copyrighted
music sample or decrypted output is committed.

The self-contained base64 fixtures under `tests/fixtures/lofty-*` are from
Serial-ATA's `lofty-rs`, licensed MIT OR Apache-2.0. They are the minimal
FLAC/Ogg/M4A vectors published at:

- `https://raw.githubusercontent.com/Serial-ATA/lofty-rs/main/lofty/tests/files/assets/minimal/full_test.flac`
- `https://raw.githubusercontent.com/Serial-ATA/lofty-rs/main/lofty/tests/files/assets/minimal/full_test.ogg`
- `https://raw.githubusercontent.com/Serial-ATA/lofty-rs/main/lofty/tests/files/assets/minimal/m4a_codec_aac.m4a`

The metadata round-trip test decodes these only into a temporary directory and
asserts container-aware audio payload fingerprints before and after tag/picture
writes; the files contain no local music sample.

The remaining dependencies are recorded in `Cargo.lock`; release packaging
must run a license audit before distribution.
