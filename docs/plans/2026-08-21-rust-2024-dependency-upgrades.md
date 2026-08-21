# Rust 2024 and Dependency Upgrades

## Status

Done

Last updated: 2026-08-21

## Objective

Migrate `unlockm` from the Rust 2021 edition to Rust 2024, raise the declared minimum supported Rust version (MSRV) to match the upgraded dependency graph, and replace materially old direct dependencies with current stable releases while preserving all CLI behavior, format compatibility, bounded processing, metadata safety, and atomic publication guarantees.

## Scope

- Update `Cargo.toml` to `edition = "2024"` and set an honest `rust-version` for the final dependency graph.
- Use Cargo's edition migration tooling and make the smallest source changes required by Rust 2024 lints or semantics.
- Audit all direct normal and development dependencies against the registry.
- Upgrade stable cross-major candidates where the project uses supported public APIs: the RustCrypto stack, `base64`, `itertools`, `lofty`, `notify`, `reqwest`, `rusqlite`, and `sha2`.
- Refresh `Cargo.lock` and remove obsolete transitive packages through normal Cargo resolution.
- Validate formatting, strict Clippy, all targets/features, locked release construction, dependency freshness, and duplicate dependency versions.

## Non-Goals

- Adopting pre-release dependencies, including `notify` 9 release candidates.
- Changing CLI flags, output layout, watch semantics, format behavior, metadata policy, cryptographic algorithms, concurrency defaults, or publication safety.
- Adding a pinned `rust-toolchain.toml`; the repository currently declares compatibility through `package.rust-version` and uses the caller's suitable stable toolchain.
- Replacing dependencies with different libraries solely to reduce dependency count.
- Removing ordinary transitive duplicates when they arise from unrelated upstream compatibility ranges.

## Current Context

- `Cargo.toml` declares Rust edition 2021 and `rust-version = "1.75"`; no `rust-toolchain*` file exists.
- The active compiler is `rustc 1.98.0`, which supports Rust 2024 migration and validation.
- `cargo outdated --workspace --root-deps-only` reports no compatible-range updates, so the current lockfile is fresh within existing semver constraints.
- Aggressive direct-dependency audit identifies stable cross-major candidates for `aes`, `base64`, `block-padding`, `cbc`, `cipher`, `itertools`, `lofty`, `md-5`, `reqwest`, `rusqlite`, and `sha2`; `notify` 8.2.0 is the latest stable line while 9.x is pre-release.
- Registry metadata reports MSRV 1.85 for current RustCrypto and Reqwest releases, and MSRV 1.89 for Lofty 0.25.1. Therefore the expected final package MSRV is 1.89, subject to final Cargo resolution and compilation.
- Architectural dependency boundaries are documented in `ARCHITECTURE.md`: Notify is behind watch orchestration, Reqwest behind metadata providers, Rusqlite behind KGM key lookup, Lofty behind tag adapters, and cryptographic crates behind `src/crypto/`.
- The worktree was clean on `main` at `20daff9` when planning began.
- The configured `/opt/homebrew/bin/sccache` must remain enabled. If sandbox loopback prevents Cargo from reaching it, required validation should be rerun with elevation rather than changing or disabling the wrapper.

## Design and Decisions

- Treat “Rust 2024” as the Rust 2024 edition migration, not a compiler-version pin. Set `rust-version` to the highest MSRV required by the selected dependency set so Cargo metadata does not promise unsupported compilers.
- Run `cargo fix --edition` while the manifest still names edition 2021, inspect its edits, then set edition 2024 and compile. Do not accept unrelated automatic rewrites.
- Prefer current stable releases. Exclude release candidates and avoid unmaintained-version churn that provides no supported stable destination.
- Upgrade coherent ecosystems together: `aes`/`cbc`/`cipher`/`block-padding` and digest crates should move as compatible groups to avoid parallel trait generations.
- Update Reqwest's obsolete `rustls-tls` feature to the supported 0.13 Rustls feature while retaining `default-features = false`, blocking mode, JSON, and a Rustls-backed TLS implementation.
- Resolve breaking API changes at their existing adapter boundaries. No dependency-specific type should leak into new architectural layers.
- Preserve behavior with regression tests rather than weakening validation or suppressing warnings.

## Milestones

### M1: Rust 2024 manifest and source migration

- Goal: compile the existing dependency graph under edition 2024 with an accurate baseline MSRV declaration.
- Dependencies: none.
- Acceptance criteria: edition migration changes are understood and scoped; `cargo check --all-targets --all-features` succeeds under edition 2024; no behavior or architecture changes are introduced.
- Validation: `rtk cargo fix --edition --all-targets --all-features --allow-dirty`; `rtk cargo check --all-targets --all-features`; focused tests for any source area changed by the migration.

### M2: Stable direct-dependency modernization

- Goal: move materially old direct dependencies to current stable lines and adapt existing boundaries to their supported APIs.
- Dependencies: M1.
- Acceptance criteria: every selected direct dependency resolves at the intended stable major/minor line; obsolete features are removed; code compiles without compatibility shims that weaken invariants; no direct stable cross-major upgrade remains unexplained.
- Validation: `rtk cargo check --all-targets --all-features`; targeted unit/integration tests for crypto, metadata, database keys, network providers, and watch mode; `rtk cargo outdated --workspace --root-deps-only --aggressive` with documented intentional exceptions.

### M3: Full regression and release validation

- Goal: prove the migration is production-ready across all repository targets.
- Dependencies: M1 and M2.
- Acceptance criteria: formatting, strict Clippy, complete test matrix, locked release build, lockfile consistency, and diff hygiene pass; the plan records exact results and any environment-only gap.
- Validation: `rtk cargo fmt --all -- --check`; `rtk cargo clippy --all-targets --all-features -- -D warnings`; `rtk cargo test --all-targets --all-features`; `rtk cargo build --release --locked`; `rtk cargo tree --locked --duplicates`; `rtk git diff --check`.

### M4: Independent implementation review and repair

- Goal: review the final diff against this plan, repair all accepted findings, and revalidate the repaired state.
- Dependencies: M3.
- Acceptance criteria: review covers edition semantics, dependency selection, API adaptations, architectural invariants, and validation evidence; no unresolved finding contradicts the objective.
- Validation: re-run every check affected by review repairs and record the review conclusion.

## Task Breakdown

- [x] T1.1 Run the Rust edition migration tooling against all targets/features, inspect its proposed source edits, and retain only required migration changes.
- [x] T1.2 Set `edition = "2024"` and the final dependency-compatible `rust-version` in `Cargo.toml`; verify all targets compile before dependency upgrades.
- [x] T2.1 Upgrade the RustCrypto/digest group in `Cargo.toml` and adapt `src/crypto/` and related tests to the new traits/APIs.
- [x] T2.2 Upgrade `base64` and `itertools`, adapting call sites and validating their focused unit tests.
- [x] T2.3 Upgrade `lofty`, adapt the metadata tag boundary, and run tag, metadata-staging, and metadata-roundtrip regressions.
- [x] T2.4 Upgrade `notify` to the latest stable line, adapt watch orchestration, and run watch-focused tests.
- [x] T2.5 Upgrade `reqwest`, preserve blocking/JSON/Rustls behavior with supported features, and run provider/no-network tests.
- [x] T2.6 Upgrade `rusqlite`, preserve bundled SQLite and KGM key lookup behavior, and run key/database tests.
- [x] T2.7 Upgrade the remaining stable direct candidates and refresh `Cargo.lock`; document any intentionally retained major version.
- [x] T3.1 Run formatting, strict Clippy, the full all-target/all-feature test matrix, and locked release build with configured sccache.
- [x] T3.2 Audit the final direct and duplicate dependency graph, inspect the complete diff, and record exact completion evidence.
- [x] T4.1 Independently review the implementation against this ExecPlan and architectural invariants.
- [x] T4.2 Repair accepted findings, rerun impacted validation plus the final required matrix, and move the plan to `Done` only when no blocking finding remains.

## Validation Strategy

- Establish a compilation baseline immediately after the edition change so edition failures are not conflated with dependency API changes.
- Upgrade coherent dependency groups one at a time and run focused tests at each boundary.
- Use existing integration suites to validate behavior: metadata staging/round-trip, no-network defaults, database keys, watch recursion/stability, fixtures, batch layout, and failure isolation.
- Finish with all targets/features because benches and integration binaries can expose edition or dependency APIs not compiled by the default target.
- Keep sccache configured. Request elevated execution when its loopback socket is denied by the sandbox; do not edit environment or repository settings to bypass it.
- Re-run `cargo outdated` after manifest/lock changes and classify any output as pre-release-only, intentionally retained with rationale, or unresolved.

## Risks and Mitigations

- **Rust 2024 changes prelude/lifetime/match ergonomics or unsafe requirements.** Use Cargo's migration lint pass first, inspect each automatic edit, and rely on strict Clippy plus all-target compilation.
- **Lofty changes metadata read/write APIs or container behavior.** Keep adaptations inside `src/metadata/tags.rs`, retain extensionless content probing, and run payload/tag round-trip regressions.
- **Notify changes event/config APIs.** Keep changes behind `src/app/watch.rs` and validate recursive, stability, coalescing, and cancellation behavior.
- **RustCrypto releases change trait names or padding ownership.** Upgrade the ecosystem together and rerun pinned parity/KDF/decrypt vectors before broader tests.
- **Reqwest feature changes accidentally restore native TLS or default features.** Continue `default-features = false` and explicitly select blocking, JSON, and supported Rustls features; inspect `cargo tree -e features` if resolution is ambiguous.
- **New dependency MSRV exceeds the declared value.** Use registry metadata and Cargo resolution to set `rust-version` to the highest actual requirement; document any dependency excluded to maintain the chosen MSRV.
- **Sandbox blocks configured sccache.** Elevate the validation command. Treat a sandbox-only failure as an environment gap until the same command passes with the wrapper still enabled.

## Progress

- [x] 2026-08-21: Read repository guidance, `docs/PLANS.md`, architecture, manifest, current active plan, and prior relevant dependency-boundary context.
- [x] 2026-08-21: Confirmed a clean `main` worktree at `20daff9`, `rustc 1.98.0`, no toolchain pin, and no duplicate active plan for this objective.
- [x] 2026-08-21: Completed compatible and aggressive direct-dependency audits and queried registry metadata for major boundary/MSRV decisions.
- [x] 2026-08-21: Implementation started with M1/T1.1, the Cargo edition migration pass.
- [x] 2026-08-21: T1.1 complete — `rtk cargo fix --edition --all-targets --all-features --allow-dirty` passed with configured sccache and proposed no source edits. Four `tail-expr-drop-order` notices were inspected at `src/app/runner.rs`, `src/app/watch.rs`, and `src/main.rs`; completed join futures/results do not own locks or unfinished work across the affected branches, so the Rust 2024 order does not require compatibility scaffolding.
- [x] 2026-08-21: T1.2/M1 complete — changed `Cargo.toml` to edition 2024 and `rust-version = "1.89"`; elevated `rtk cargo check --all-targets --all-features` passed with configured sccache before any dependency version changes.
- [x] 2026-08-21: T2.1 complete — upgraded `aes` 0.8→0.9, `cbc` 0.1→0.2, `cipher` 0.4→0.5, `block-padding` 0.3→0.4, `md-5` 0.10→0.11, and dev `sha2` 0.10→0.11. Adapted new `Array`, block cipher/mode traits, padding errors/methods, explicit ECB block unpadding, and digest hex encoding. Elevated all-target/all-feature check passed; `rtk cargo test crypto::` passed 24 tests and `rtk cargo test keys::` passed 4 tests.
- [x] 2026-08-21: T2.2 complete — upgraded `base64` 0.22→0.23 and `itertools` 0.14→0.15 without source changes. Elevated all-target/all-feature check passed and `rtk cargo test formats::` passed 13 tests covering the affected footer/format parsing paths.
- [x] 2026-08-21: T2.3 complete — upgraded `lofty` 0.21→0.25 and replaced removed `Picture::new_unchecked` construction by reusing the already validated parsed picture and setting its type to front cover. Elevated all-target/all-feature check passed; `rtk cargo test metadata::` passed 14 tests and metadata staging/round-trip integration passed 6 tests across 2 suites.
- [x] 2026-08-21: T2.4 complete — upgraded `notify` 6.1→8.2, the latest stable line, without source changes. Elevated all-target/all-feature check passed; watch state-machine tests passed 3 and recursive/stability integration passed 4 tests across 2 suites.
- [x] 2026-08-21: T2.5 complete — upgraded `reqwest` 0.12→0.13 and replaced its obsolete `rustls-tls` feature with supported `rustls`, retaining disabled defaults plus blocking and JSON. Elevated all-target/all-feature check passed; metadata provider mock/default-no-network integration passed 2 tests across 2 suites.
- [x] 2026-08-21: T2.6 complete — upgraded `rusqlite` 0.32→0.40 with bundled SQLite preserved. Elevated all-target/all-feature check passed and `rtk cargo test keys::` passed 4 database/MMKV tests.
- [x] 2026-08-21: T2.7/M2 complete — Cargo resolution refreshed every selected dependency in `Cargo.lock`. Final compatible direct audit is empty. Final aggressive audit contains only `notify` 9.0.0-rc.4, intentionally retained at stable 8.2.0. Locked direct resolution includes `aes` 0.9.2, `base64` 0.23.1, `block-padding` 0.4.2, `cbc` 0.2.1, `cipher` 0.5.2, `itertools` 0.15.0, `lofty` 0.25.1, `md-5` 0.11.0, `notify` 8.2.0, `reqwest` 0.13.4, `rusqlite` 0.40.2, and dev `sha2` 0.11.0.
- [x] 2026-08-21: T3.1 complete — `rtk cargo fmt --all -- --check`, elevated `rtk cargo clippy --all-targets --all-features -- -D warnings`, elevated `rtk cargo test --all-targets --all-features`, elevated `rtk cargo build --release --locked`, and `rtk git diff --check` passed with configured sccache retained. Initial strict Clippy found 10 new-toolchain mechanical lints; equivalent let-chain, `is_none_or`, `is_multiple_of`, and fixed-array chunk forms were applied and the rerun reported no issues.
- [x] 2026-08-21: T3.2/M3 complete — final compatible/aggressive direct audits, locked direct tree, duplicate tree, complete source/manifest diff, status, formatting, and diff hygiene were inspected. Duplicate tree contains only Reqwest's transitive `base64` 0.22 beside direct 0.23 and upstream proc-macro `syn` 2/3; neither is actionable from this manifest. Local toolchains are only stable 1.98 and nightly, so declared MSRV 1.89 is supported by registry metadata and current compilation but was not independently compiled with rustc 1.89.
- [x] 2026-08-21: T4.1-T4.2/M4 complete — independent plan/diff review passed with notes and no blocker, major, or meaningful minor finding. Additional locked MSRV, Reqwest feature, crypto-mode/error, Lofty picture, scope, and validation-evidence audits passed. No code repair was required; the only retained note is the unavailable rustc 1.89 exact-floor run.

## Discoveries

- Compatible-range dependencies are already current, so meaningful upgrades require intentional manifest changes rather than only `cargo update`.
- `cargo outdated` reports `notify` 9.0.0-rc.4 as latest; the stable target is 8.2.0 under the no-pre-release decision.
- Reqwest 0.13 marks the old `rustls-tls` feature obsolete and exposes `rustls` as the supported Rustls-backed feature.
- Lofty 0.25.1 declares Rust 1.89, raising the expected project MSRV above Rust 2024's language minimum of 1.85.
- Cargo's edition migration lint pass produced no source rewrite. Its only notices concern earlier destruction of already-completed async temporary futures/results relative to branch locals; the durable runner, presentation, cancellation, and job-set owners retain their explicit surrounding scopes.
- RustCrypto 0.5 removes the old padded-decrypt convenience from bare block ciphers. NCM's ECB payloads now explicitly split exact AES blocks, decrypt them in place, reject a non-block tail, and apply PKCS#7 unpadding; fixed NCM vectors remain green.
- Rustfmt under edition 2024 applies the edition's current style rules repository-wide, including import ordering and multiline expression layout. These are formatting-only changes produced by the required final `cargo fmt`; functional changes remain confined to manifest/lock updates, dependency API boundaries, and strict-Clippy equivalents.
- The final duplicate graph is not fully singular: Reqwest still transitively requires `base64` 0.22 while the project directly uses 0.23, and upstream procedural macros resolve both `syn` 2 and 3. No direct version choice can remove these without downgrading selected stable releases.

## Decision Log

- **2026-08-21 — Edition plus honest MSRV.** Migrate to edition 2024 and declare the highest MSRV required by selected direct dependencies instead of pinning the local 1.98 compiler.
- **2026-08-21 — Stable releases only.** Upgrade to stable cross-major releases and explicitly stop Notify at 8.2.0 while 9.x remains a release candidate.
- **2026-08-21 — Preserve dependency boundaries.** Adapt breaking APIs in existing crypto, metadata, watch, provider, and key modules without changing product semantics.
- **2026-08-21 — Preserve sccache.** Use elevation for sandbox-denied wrapper access; do not clear, disable, or reconfigure the cache.

## Completion Evidence

Implementation validation is complete and the plan is in review.

- Manifest: Rust edition 2024, package MSRV 1.89, all identified stable direct cross-major dependencies upgraded, and Reqwest configured with supported blocking/JSON/Rustls features while defaults remain disabled.
- Behavioral adaptations: RustCrypto 0.5 block/mode/padding traits, explicit NCM ECB block unpadding, digest hex encoding, Lofty 0.25 parsed-picture reuse, and mechanical current-compiler lint equivalents.
- Focused validation: crypto 24 passed; keys 4 passed (both RustCrypto and Rusqlite phases); formats 13 passed; metadata unit 14 passed; metadata staging/round-trip 6 across 2 suites; watch unit 3 and recursive/stability 4 across 2 suites; provider/no-network 2 across 2 suites.
- Final validation: formatting check passed; strict Clippy all targets/features passed with no issues; full all-target/all-feature test command passed; locked release build passed; diff check passed; compatible direct outdated audit empty; aggressive audit only shows intentionally excluded Notify 9 RC.
- Dependency graph: locked direct versions recorded in Progress. Duplicate audit shows only upstream-compatible `base64` 0.22/0.23 and `syn` 2/3.
- Environment/MSRV note: all Cargo validations used the configured sccache with elevated access where required. rustc 1.98 passed; rustc 1.89 is not installed locally, so exact-floor compilation remains unverified.
- Review: Pass with Notes. No code finding required repair; final implementation validation remains green and the exact rustc 1.89 floor run is the only explicit gap.

## Review Notes

- [x] 2026-08-21 — Pass with Notes. No blocker, major, or meaningful minor finding. The implementation matches the objective and preserves dependency boundaries and architectural invariants.
- [x] Dependency/MSRV review: locked `cargo metadata` shows the highest transitive declared MSRV is exactly 1.89 (`lofty`, `lofty_attr`, and `ogg_pager`); the new standard APIs used by Clippy repairs stabilized no later than 1.88.
- [x] TLS review: `cargo tree -e features -i reqwest --locked` shows only blocking, JSON, and Rustls features from `unlockm`; no Reqwest default/native-TLS feature is enabled. macOS `security-framework` is reached through the Rustls platform verifier, not native-tls.
- [x] Correctness review: RustCrypto adaptations preserve CBC and bare-ECB distinctions, reject incomplete AES blocks before PKCS#7 unpadding, retain original error mapping, and are covered by fixed vectors. Lofty reuses the already parsed/validated picture bytes and MIME while changing only the picture type to front cover.
- [x] Scope review: edition-2024 rustfmt causes repository-wide formatting-only changes; the functional diff remains at manifest/lock, dependency adapters, and current-compiler lint equivalents. The churn is accepted because normal `cargo fmt` under the migrated manifest reproduces it.
- [x] Validation note: rustc 1.89 is not installed locally, so exact-floor compilation is not independently executed. This is an explicit non-blocking gap because registry metadata, locked metadata, source API stabilization, rustc 1.98 full validation, and the package declaration agree on 1.89.

## Outcomes & Retrospective

`unlockm` now uses Rust edition 2024 with an honest 1.89 MSRV and current stable direct dependency lines. The migration retained every existing architectural boundary: crypto updates remain in local crypto/key modules, Lofty remains behind metadata tags, Notify behind watch orchestration, Reqwest behind providers, and Rusqlite behind KGM keys. Existing fixed vectors and focused integration suites caught and validated the few required public-API adaptations.

The key implementation lesson is that a direct dependency audit must include aggressive cross-major comparison: the original lockfile was current within semver constraints but still hid twelve stable major-line upgrades. Upgrading coherent ecosystems together avoided duplicate crypto trait generations. Rust 2024 also changes rustfmt's default style, so repository-wide formatting churn is expected even when source semantics need no edition rewrite.

The final state passes formatting, strict Clippy, the complete all-target/all-feature test matrix, a locked release build, dependency freshness checks, duplicate-tree review, and diff hygiene with configured sccache. Notify intentionally remains at stable 8.2.0 instead of 9.0.0-rc.4. The exact MSRV compiler is not installed locally; locked metadata proves no dependency declares more than 1.89, but a future CI job using rustc 1.89 would close that last validation note.
