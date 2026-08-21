# Third-party notices

`src/crypto/` selectively incorporates the audio probing and NCM, QMC, KGM,
Kuwo, Xiami, and Ximalaya primitives that `unlockm` uses from the author's
`lib_um_crypto_rust` revision
`5dad838c33c3e4aab198d995160d3fbb9968470f`. Those sources are licensed MIT OR
Apache-2.0; substantial local modules retain source-level attribution. The
release has no build or runtime dependency on that repository.

The MMKV reader and macOS QQMusic discovery logic in `src/keys/qmc.rs` are a
small Rust port of `github.com/unlock-music/go-mmkv` v0.1.4 and the Go
reference's `cli/internal/mmkv` / `cli/algo/qmc/qmmac` code. Those sources are
MIT licensed; the port retains their AES-128-CFB, CRC-IV, container, and macOS
derivation behavior.

The deterministic QMC footer/static-key and NCM parser/stream vectors in
`src/formats.rs`, `tests/support/`, and `tests/metadata_staging.rs` are textual
copies of
`um_crypto/qmc/src/footer/fixtures/ekey_pc_enc_v1.bin` and
`um_crypto/ncm/src/__fixture__/ncm_test1.bin` at that revision. They are
provided by Project Unlock Music under MIT and Apache-2.0; no local copyrighted
music sample or decrypted output is committed.

The base64 X2M/X3M encrypted/plain header vectors under `tests/fixtures/` come
from `um_crypto/xmly/src/__fixture__/` at the same pinned revision. They contain
only 1 KiB format headers and are used to prove the local Android permutation
and XOR transforms.

The base64 KGM v2/v3/invalid-magic header vectors and QMC PCv1/PCv2 MusicEx
footer vectors under `tests/fixtures/` come from the corresponding
`um_crypto/kgm/src/__fixtures__/` and `um_crypto/qmc/src/footer/fixtures/`
files at the same pinned revision. They validate header versions, bounded
transforms, EKey/footer decoding, and metadata identity without music payloads.

The self-contained base64 fixtures under `tests/fixtures/lofty-*` are from
Serial-ATA's `lofty-rs`, licensed MIT OR Apache-2.0. They are the minimal
FLAC/Ogg/M4A vectors published at:

- `https://raw.githubusercontent.com/Serial-ATA/lofty-rs/main/lofty/tests/files/assets/minimal/full_test.flac`
- `https://raw.githubusercontent.com/Serial-ATA/lofty-rs/main/lofty/tests/files/assets/minimal/full_test.ogg`
- `https://raw.githubusercontent.com/Serial-ATA/lofty-rs/main/lofty/tests/files/assets/minimal/m4a_codec_aac.m4a`

The metadata round-trip test decodes these only into a temporary directory and
asserts container-aware audio payload fingerprints before and after tag/picture
writes; the files contain no local music sample.

`tests/fixtures/lofty-silent.mp3.b64` is the deterministic silent MPEG fixture
from Project Unlock Music's `um_audio/src/__fixtures__/ffmpeg_silent.mp3` at
the pinned revision, licensed MIT OR Apache-2.0. It is stored as base64 text so
the test suite does not require a sibling checkout.

The remaining dependencies are recorded in `Cargo.lock`; release packaging
must run a license audit before distribution.
