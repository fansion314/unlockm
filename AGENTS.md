# Repository guidance

- Prefix shell commands with `rtk` as required by the parent environment guidance.
- `ARCHITECTURE.md` is optional reading for small, local edits and recommended before changing CLI behavior, traversal, watch mode, format adapters, key sources, metadata, output safety, or concurrency.
- For planned work, read `docs/PLANS.md` and the active file under `docs/plans/` before implementation. Keep plan progress, discoveries, and validation evidence current as work proceeds.
- Preserve the architectural invariants around bounded streaming, path containment, collision handling, conservative metadata updates, source retention, and atomic output publication.
- Keep format-specific logic behind adapters; batch and watch modes must use the same path planner and per-file processing pipeline.
- Update `ARCHITECTURE.md` when an implementation changes a major boundary, invariant, extension point, or execution flow.
