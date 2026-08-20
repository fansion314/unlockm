# ExecPlan Standard

An ExecPlan is the durable handoff for a non-trivial change. It must let another contributor implement, validate, review, and resume the work without relying on chat history.

## Location and naming

Store plans in `docs/plans/YYYY-MM-DD-short-kebab-slug.md`. Update an existing active plan instead of creating a duplicate for the same objective.

## Required sections

Every plan contains these sections in this order:

1. `Title`
2. `Status`
3. `Objective`
4. `Scope`
5. `Non-Goals`
6. `Current Context`
7. `Design and Decisions`
8. `Milestones`
9. `Task Breakdown`
10. `Validation Strategy`
11. `Risks and Mitigations`
12. `Progress`
13. `Discoveries`
14. `Decision Log`
15. `Completion Evidence`

Use `Draft` when a material decision is unresolved, `Ready` when implementation can begin, `In Progress` during implementation, `In Review` after implementation validation, and `Done` only after review findings are resolved or explicitly accepted.

## Milestones and tasks

Milestones describe user-visible or architectural increments. Each milestone states its goal, dependencies, acceptance criteria, and validation commands or manual checks.

The task breakdown uses stable identifiers such as `T1.1`. Tasks are commit-sized, name concrete files or modules, and include a verification step. Check a task only when its implementation and stated verification are complete.

## Living-document rules

- The implementer updates `Status`, `Progress`, `Discoveries`, `Decision Log`, and `Completion Evidence` throughout the work.
- Record deviations before or with the code that depends on them. Do not silently change chosen product semantics.
- Include exact commands and their outcomes. A skipped platform, unavailable fixture, or missing external service is a validation gap, not a pass.
- Keep unresolved failures and review findings visible.
- Prefer links to source paths and tests over pasted implementation code.
- Plans document intended commits, but do not claim a commit exists until its hash is recorded in `Progress` or `Completion Evidence`.

## Completion bar

A plan may move to `Done` only when all in-scope tasks and milestone acceptance criteria are satisfied, required validation is recorded, documentation matches the implemented behavior, and no unresolved finding contradicts the objective.
