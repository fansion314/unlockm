# Full Workspace Dependency Refresh

## Status

In Review

Last updated: 2026-08-25

## Objective

Upgrade every stable dependency used by the `unlockm` Rust workspace and the `unlockm-desktop` Tauri/React application, refresh both lockfiles, adapt only the compatibility breakage required by those upgrades, and preserve all existing CLI, desktop, security, packaging, and output-safety behavior.

## Scope

- Audit and refresh all direct, development, build, and transitive Rust dependencies for the root `unlockm` crate and `desktop/src-tauri` workspace member.
- Upgrade all production and development dependencies in `desktop/package.json` to their latest stable releases through Vite+, including cross-major upgrades.
- Upgrade the `packageManager`-pinned pnpm backend to its latest stable release through Vite+.
- Keep the `vite` npm alias, `vite-plus`, and the `overrides.vite` target on one coherent Vite+ release.
- Refresh `Cargo.lock` and `desktop/pnpm-lock.yaml` using the repository-approved Cargo and Vite+ entry points.
- Make the smallest source/config/test changes required by dependency API, typing, lint, formatting, or build changes.
- Re-audit freshness and validate the complete Rust, frontend, Tauri host, license, and security gates.

## Non-Goals

- Adopting alpha, beta, release-candidate, canary, nightly, or other pre-release packages.
- Updating GitHub Actions merely because they are externally pinned tooling rather than application dependencies.
- Changing application features, CLI flags, database semantics, conversion behavior, UI design, permissions, CSP, bundling targets, or release signing policy.
- Replacing libraries with unrelated alternatives or removing dependencies solely to reduce the graph.
- Creating a commit, tag, release, or push unless separately requested.

## Current Context

- The worktree is clean on `main` at tagged release `v0.3.2` before this plan is added.
- The Rust workspace contains `unlockm` and `unlockm-desktop`, shares root `Cargo.lock`, declares edition 2024 and MSRV 1.89, and must keep the configured sccache wrapper enabled.
- `desktop/package.json` and `desktop/pnpm-lock.yaml` are exclusively managed through `vp`; raw npm, pnpm, yarn, and bun lifecycle commands are forbidden.
- The 2026-08-25 aggressive direct Rust audit reports no stable direct upgrade. Only `notify 9.0.0-rc.4` is newer than stable `8.2.0`, so it is excluded. A normal `cargo update` is still required to refresh compatible transitive releases.
- The frontend audit reports stable upgrades for `@testing-library/user-event 14.6.5→14.6.6`, `@types/react-dom 19.2.4→19.2.5`, `vitest 4.1.10→4.1.11`, `@types/node 24.13.3→26.2.0`, `react-i18next 16.5.8→17.0.12`, `typescript 5.9.3→7.0.2`, and the Vite+ alias/toolchain pair `0.2.8→0.3.0`.
- Existing CI uses Node 24 through `setup-vp`, Rust 1.89, frozen frontend installs, strict Clippy, full Rust/frontend tests, a native Tauri debug build, and packaged-license/security audits.

## Design and Decisions

- “All dependencies” means every stable registry update, including cross-major frontend releases and compatible transitive lockfile refreshes. Pre-releases remain out of scope.
- Use `rtk cargo update` for the shared Rust lock and `rtk vp -C desktop update --latest` for frontend manifest/lock changes. Do not bypass Vite+ with a raw package manager.
- Keep Tauri Rust crates and JavaScript packages compatible as an ecosystem; freshness audits currently show their direct pins are already latest stable.
- Prefer the newest transitive version that is jointly resolvable under all upstream semver/exact constraints. Do not patch or replace upstream crates merely to bypass an exact transitive pin.
- Preserve MSRV 1.89 if the refreshed graph supports it. If a selected stable dependency requires a higher compiler, update both package declarations and CI toolchain pins together and record the reason before proceeding.
- Keep dependency adaptations behind existing Rust adapters and React/Tauri boundaries. Dependency upgrades must not broaden native capabilities, filesystem reach, network defaults, or output mutation semantics.
- Treat Vite+ 0.3, TypeScript 7, React-i18next 17, and Node 26 types as the highest-risk group and validate formatting, lint/type checking, tests, and production build immediately after update.

## Milestones

### M1: Rust workspace refresh

- Goal: resolve the newest stable compatible Rust graph for both workspace packages.
- Dependencies: none.
- Acceptance criteria: direct stable audit is empty, pre-release-only exceptions are documented, the lockfile is refreshed, and the workspace compiles without weakening features or MSRV truthfulness.
- Validation: `rtk cargo update`; `rtk cargo outdated --workspace --root-deps-only --aggressive`; `rtk cargo check --locked --workspace --all-targets --all-features`.

### M2: Desktop frontend dependency refresh

- Goal: upgrade every production/dev frontend dependency to the latest stable release and reconcile breaking toolchain/API changes.
- Dependencies: M1 only for orderly lockfile review.
- Acceptance criteria: `package.json` and `pnpm-lock.yaml` resolve the audited latest stable versions; Vite+ alias/override versions agree; frozen install, format, lint/type, tests, and production build pass.
- Validation: `rtk vp -C desktop install --frozen-lockfile`; `rtk vp -C desktop fmt . --check`; `rtk vp -C desktop check --no-fmt`; `rtk vp -C desktop test`; `rtk vp -C desktop build`.

### M3: Cross-stack release validation

- Goal: prove the refreshed dependency graph preserves the complete application and native-host contract.
- Dependencies: M1 and M2.
- Acceptance criteria: strict Rust lint/test gates, desktop license/security tests, Tauri debug app build, dependency freshness audits, lockfile determinism, and diff hygiene all pass.
- Validation: full commands under Validation Strategy, including `rtk vp -C desktop exec tauri build --debug --bundles app` on macOS.

## Task Breakdown

- [x] T1.1 Refresh compatible Rust direct/transitive resolution in `Cargo.lock` and inspect the exact graph change.
- [x] T1.2 Re-run stable/aggressive direct audits, verify MSRV compatibility, and compile all workspace targets/features with the locked graph.
- [x] T2.1 Run the Vite+-managed all-package latest update and reconcile `desktop/package.json`, its Vite alias/override, pnpm backend pin, and `desktop/pnpm-lock.yaml`.
- [x] T2.2 Adapt frontend source, configuration, and tests for Vite+ 0.3, TypeScript 7, React-i18next 17, Node 26 types, and other upgraded packages.
- [x] T2.3 Complete frozen install plus frontend format/check/test/build and re-run the frontend outdated audit.
- [x] T3.1 Run Rust formatting, strict workspace Clippy, and complete all-target/all-feature tests with sccache retained.
- [x] T3.2 Run desktop Rust license/security tests and build the native Tauri debug `.app` bundle.
- [x] T3.3 Audit final Rust/frontend freshness, dependency duplicates, manifests/locks, and diff hygiene; record final evidence and move the plan to `In Review`.

## Validation Strategy

- Rust: `rtk cargo fmt --all -- --check`; elevated `rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`; elevated `rtk cargo test --locked --workspace --all-targets --all-features`; `rtk cargo tree --locked --duplicates`.
- Frontend: `rtk vp -C desktop install --frozen-lockfile`; `rtk vp -C desktop fmt . --check`; `rtk vp -C desktop check --no-fmt`; `rtk vp -C desktop test`; `rtk vp -C desktop build`.
- Desktop native: `rtk cargo test --locked -p unlockm-desktop --test license_audit`; `rtk cargo test --locked -p unlockm-desktop --test security_policy`; elevated `rtk vp -C desktop exec tauri build --debug --bundles app`.
- Freshness: `rtk cargo outdated --workspace --root-deps-only --aggressive`; `rtk vp -C desktop outdated --format list`; classify any remaining output as pre-release-only, ecosystem-constrained, or unresolved.
- Integrity: frozen/locked commands, Vite alias/override equality, version consistency, `rtk git diff --check`, and complete status/diff review.

## Risks and Mitigations

- **TypeScript 7 or Vite+ 0.3 changes compiler, formatter, linter, or test behavior.** Upgrade the coherent Vite+ alias/toolchain group together, run each frontend gate separately, and accept only required source/config changes.
- **React-i18next 17 changes hooks or typing.** Keep adaptations at the existing i18n bootstrap/components and protect locale behavior with the current locale and component tests.
- **Node 26 types expose APIs not available under CI Node 24.** Treat types as compile-time declarations only; retain Node 24 runtime compatibility and reject source changes that depend on Node 26-only runtime behavior.
- **Transitive Rust refresh raises MSRV or duplicates ecosystems.** Inspect locked metadata and duplicate trees; preserve 1.89 when possible or update declarations/CI together with evidence.
- **Tauri ecosystem mismatch breaks native IPC or packaging.** Keep matching current stable Rust/JavaScript plugin lines and finish with a real macOS Tauri app bundle.
- **Sandbox blocks registry access or sccache.** Use approved elevation while retaining configured tools; never disable sccache or bypass Vite+.

## Progress

- [x] 2026-08-25: T2.1 re-closed after explicitly upgrading `packageManager` from pnpm 11.22.0 to registry-latest 11.23.0. Vite+ downloaded/used pnpm 11.23.0; frozen strict-peer install passed, then local Vite+ 0.3 formatting (58 files), check (40 files), tests (17 files/71 tests), and production build passed again.
- [x] 2026-08-25: T3.3/M3 complete. Cargo dry-run locks zero packages; verbose output lists only four non-selected transitive versions, all blocked by exact/joint upstream resolution. Rust aggressive direct audit contains only Notify 9 RC; frontend direct outdated output is empty; pnpm registry/latest and manifest are both 11.23.0; Vite direct alias, local tool, and override all equal 0.3.0. Complete frontend audit, including dev dependencies, reports no known vulnerabilities. Final diff is limited to `Cargo.lock`, `desktop/package.json`, `desktop/pnpm-lock.yaml`, and this plan; diff hygiene passes.
- [x] 2026-08-25: Targeted `toml 0.8.23` resolution intentionally failed without mutating the lock because its `toml_datetime ^0.6.11` requirement conflicts with Tauri's GTK/WebKit path through `proc-macro-crate 2.0.2` and exact `toml_datetime =0.6.3`. Initial `cargo info` queries also failed because crates.io is replaced by the configured registry; explicit `--registry crates-io` retries succeeded and established the MSRV facts. These are documented constraint checks, not unresolved validation failures.
- [x] 2026-08-25: T3.3 toolchain audit discovered `packageManager: pnpm@11.22.0` trails stable 11.23.0. T2.1 is temporarily reopened so the explicitly pinned backend is included in the full refresh before final validation.
- [x] 2026-08-25: T3.2 complete. License audit (1) and security-policy suite (3) passed inside the full Rust matrix. Elevated `rtk vp -C desktop exec tauri build --debug --bundles app` rebuilt the frontend with the upgraded graph, compiled the native host with sccache, and produced `target/debug/bundle/macos/UnlockM.app`; Info.plist reports 0.3.2 and `world.fansionia.unlockm`, the arm64 executable is present, and both bundled license resources are present.
- [x] 2026-08-25: T3.1 complete. With sccache retained through the approved elevated shell, Cargo formatting, strict locked workspace Clippy `-D warnings`, and the complete locked workspace all-target/all-feature test command passed. This includes 116 core unit tests, all CLI/integration/watch suites, both throughput/metadata benches, 55 desktop unit tests, one license audit, and three security-policy tests with zero failures.
- [x] 2026-08-25: T2.2-T2.3/M2 complete with no source compatibility patch required. Using local `vp 0.3.0`, format passed on 58 files, lint/type check passed on 40 files, 17 test files/71 tests passed under Vitest 4.1.11, and the production build passed under Vite 8.2.2/Rolldown 1.2.5 with only the pre-existing dynamic-import/chunk-size advisories. Final frontend outdated output is empty. Production package audit reports no known vulnerabilities. The first audit invocation used `vp pm audit --prod`, which Vite+ rejected as CLI syntax; corrected `vp pm audit -- --prod` passed.
- [x] 2026-08-25: T2.1 complete. `rtk vp -C desktop update --latest` upgraded all audited frontend packages plus compatible manifest ranges for React types/plugin-react, replacing 32 packages with 33. The updater left `overrides.vite` on 0.2.8 while direct Vite+/alias moved to 0.3.0; the override was explicitly synchronized and lock resolution rerun. Top-level list now shows 23 intended packages, local `vp` reports 0.3.0 with Vite 8.2.2/Rolldown 1.2.5, and frozen install with strict peer dependencies passes.
- [x] 2026-08-25: T1.2/M1 complete. Elevated locked workspace check passed for every target/feature. Aggressive direct audit contains only excluded pre-release Notify 9 RC. Locked metadata's highest declared dependency MSRV is exactly 1.89 (`lofty`, `lofty_attr`, `ogg_pager`), so package/CI declarations remain truthful. Duplicate audit remains only upstream `base64 0.22/0.23` and `syn 2/3`.
- [x] 2026-08-25: T1.1 complete. `rtk cargo update` advanced 11 compatible transitive releases and consolidated the TOML 0.8 subgraph to the newest jointly resolvable versions. The final targeted resolution check confirmed that the remaining older nodes are upstream-constrained rather than missed updates.
- [x] 2026-08-25: Implementation started at M1/T1.1. Local validation runtime is rustc/cargo 1.98.0 with configured sccache and Node 24.19.0, which satisfies Vite+ 0.3's Node requirement.
- [x] 2026-08-25: Read repository guidance, `docs/PLANS.md`, the prior completed Rust dependency plan, manifests, CI/release workflows, Vite+ dependency commands, and current clean Git state.
- [x] 2026-08-25: Completed initial registry audits. Rust direct dependencies are current on stable lines with only Notify 9 RC excluded; frontend has eight stable package updates, including four ecosystem/major transitions.

## Discoveries

- Vite+ 0.3/Vitest 4.1.11 transitively retains `@testing-library/user-event 14.6.5` through `@vitest/browser-preview`, while the project direct dev dependency is latest 14.6.6. This exact upstream transitive edge explains the two lock entries and is not actionable from the application manifest.
- Vite+ dependency update does not advance the `packageManager` field. A separate registry query reports pnpm 11.23.0, so the backend pin requires an explicit manifest update and frozen-install revalidation.
- Despite the cross-major transitions, current application code is already compatible with TypeScript 7, React-i18next 17, Node 26 declarations, Vitest 4.1.11, and Vite+ 0.3; no source/test/config edit beyond dependency declarations is required.
- Vite+ 0.2's all-latest updater updates the direct `vite` npm alias and `vite-plus` dependency but does not update an existing `overrides.vite`; leaving it untouched would silently split the declared and forced core versions. The override must be reconciled manually before accepting the lock.
- `generic-array 0.14.7`, `toml_edit 0.20.2`, and `toml_datetime 0.6.3` are exact upstream requirements. Registry metadata shows `toml 0.8.23` itself supports Rust 1.66, but it requires `toml_datetime ^0.6.11`; Tauri's GTK/WebKit chain reaches `proc-macro-crate 2.0.2`, which requires `toml_datetime =0.6.3`. Cargo must unify this semver-compatible 0.6 line, so `cargo update -p toml@0.8.2 --precise 0.8.23` correctly fails resolution and leaves the lock unchanged. `toml 0.8.2` is therefore the newest jointly resolvable 0.8 release in this graph, not an ignored stable upgrade.
- The Rust manifests are already current at the direct stable level; the Rust portion is primarily a compatible transitive lock refresh plus proof that no new stable direct release was missed.
- Vite+ `outdated --format json` includes complete registry manifests and returns exit 1 when updates exist; `--format list` is the appropriate concise evidence format.
- Vite+ 0.3 requires Node `^20.19 || ^22.18 || >=24.11`; the project CI's Node 24 line is compatible as long as setup resolves a current Node 24 patch.

## Decision Log

- **2026-08-25 — Stable-only full refresh.** Accept every stable dependency update, including frontend cross-major releases, but exclude Notify 9 RC and all other pre-releases.
- **2026-08-25 — Preserve tool boundaries.** Use Cargo and Vite+ as the only lock/manifest mutation paths and keep sccache enabled.
- **2026-08-25 — Preserve runtime contracts.** Dependency changes may require compatibility edits but do not authorize new product behavior, native permissions, or Node 26 runtime assumptions.
- **2026-08-25 — Respect joint upstream resolution.** Keep `toml 0.8.2` and the exact-pinned `toml_datetime 0.6.3`/`toml_edit 0.20.2`/`generic-array 0.14.7` nodes because the current stable upstream graph cannot resolve their advertised newer versions together. Do not patch transitive manifests merely to manufacture a nominally newer lock.

## Completion Evidence

Implementation and validation are complete; the plan is ready for independent review.

- Rust direct dependencies for both `unlockm` and `unlockm-desktop` are current stable. `Cargo.lock` refreshes the compatible transitive graph; final dry-run locks zero packages and the only aggressive direct result is excluded pre-release Notify 9 RC.
- Frontend direct dependencies and toolchain pins are current stable: React-i18next 17.0.12, Testing Library user-event 14.6.6, React DOM types 19.2.5, Node types 26.2.0, plugin-react 6.1.0, TypeScript 7.0.2, Vitest 4.1.11, Vite+/core 0.3.0, and pnpm 11.23.0. Final frontend outdated output is empty.
- Vite alias integrity is explicit: `devDependencies.vite` and `overrides.vite` both target `npm:@voidzero-dev/vite-plus-core@0.3.0`, while `vite-plus` is 0.3.0. Frozen strict-peer installation passes.
- Frontend validation: format 58 files, lint/type 40 files, 17 test files/71 tests, and production build pass under the upgraded graph. Full and production-only audits report no known vulnerabilities. Existing ineffective-dynamic-import and >500 kB chunk notices remain non-failing pre-existing advisories.
- Rust validation: format, strict locked workspace Clippy, locked all-target/all-feature tests and benches pass with sccache retained. The matrix includes 116 core unit tests, all CLI/integration/watch suites, 55 desktop unit tests, one license audit, and three security tests.
- Native validation: Tauri debug `.app` build passes at `target/debug/bundle/macos/UnlockM.app`; version 0.3.2, identifier, arm64 executable, and bundled license resources are correct.
- Remaining older transitive nodes are explicitly upstream constrained: `generic-array 0.14.7`, `toml_datetime 0.6.3`, `toml_edit 0.20.2`, `toml 0.8.2`, and Vite+'s transitive user-event 14.6.5. No application-level override can advance them without breaking current stable upstream resolution.

## Outcomes & Retrospective

The complete application dependency surface now resolves to the latest stable versions available to the project, including the often-missed Vite alias override and `packageManager` backend pin. No production source or test behavior needed adaptation: all changes are isolated to manifests, lockfiles, and this durable plan.

The main lesson is that updater output alone is insufficient for a full refresh. Vite+ did not synchronize the existing override or pnpm pin, and Cargo advertised newer individual transitive crates that cannot coexist under the current Tauri graph's exact semver-compatible pins. Explicit alias equality, backend queries, targeted resolution attempts, and final frozen/locked validation distinguish real omissions from upstream constraints.
