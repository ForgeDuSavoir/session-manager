# To Do

This is the active task list for the [current implementation plan](docs/implementation-plan.md), [functional specification](docs/functional-specification.md), and [replacement technical contract](docs/technical-specification.md#replacement-contract-directory-backed-projects-and-application-resolution). Complete the stages in order; the initial release ends at C3. Tasks (L1) and pause/resume (L2) do **not** block it. The [historical task list](ToDo-legacy.md) preserves the previous M0–M6 work and checkmarks; its completed tasks are not proof that the replacement is implemented.

## Current status and reuse

- [x] Establish the Rust package, safe test foundation, naming primitives, durable runtime/checkpoint primitives, and the earlier M0–M2 evidence **for the previous central-profile contract**. See [historical acceptance records](docs/implementation-plan.md#historical-acceptance-records); do not treat these as C0–C3 acceptance.
- [x] Document the directory-backed functional intent and [ADR 0012](docs/decisions/0012-discover-project-profiles-and-resolve-xdg-applications.md).
- [x] Replace the legacy roadmap with staged C0–C3/L1–L2 acceptance criteria, preserving the [old plan](docs/legacy-implementation-plan.md).
- [x] Audit existing M1/M2 code and tests against the replacement contract before selecting reusable pieces. See the [C0 reuse audit](docs/reviews/c0-contract-review.md#m1m2-reuse-audit). Preserve valid UUID/numbering, atomic-storage, and safety code where appropriate; record obsolete central-profile CRUD, task-in-TOML, root/app snapshots, rename propagation, deletion guards, and old public projection/operation assumptions for removal or replacement. Do not assume a checked historical item remains valid.

## C0 — Settle the replacement contracts before implementation

**Accepted:** the [version 2 contract](docs/technical-specification.md#initial-release-technical-contract-version-2) and [C0 review evidence](docs/reviews/c0-contract-review.md) record owner approval, with no legacy compatibility/migration requirement. C1 may begin.

References: [C0 scope and gate](docs/implementation-plan.md#c0--complete-the-minimum-replacement-contract) and [replacement technical contract](docs/technical-specification.md#replacement-contract-directory-backed-projects-and-application-resolution).

- [x] Specify the project-source setting, directory/name validation, immediate-child discovery, missing/renamed/reused-name behavior, and start-time lookup without a persistent path link or project snapshot.
- [x] Specify template discovery, selection, default empty `Default Profile`, first-free `Name-N` allocation, symlinks, safe copy limits, race/error handling, and cleanup or recovery of incomplete creations; never merge or overwrite existing projects.
- [x] Specify the exclusion file's path and format, desktop-entry IDs, XDG search/priority and visibility rules, project-local override and reintroduction, invalid file handling, result limits, and no-active/missing-project fallback.
- [x] Define the new versioned public API and CLI contract for project/template listing, project creation, running-session list/start/switch/stop, and current application listing. Specify errors, preconditions, revisions/events only when needed; do not silently reuse incompatible version 1 method shapes.
- [x] Define minimal runtime persistence without central profile foreign keys, root/app snapshots, task data, or adapter startup requirements. Define the clean-break policy: ignore old configuration, sessions, transactions and checkpoints; no migration, recovery or continued usability requirement, and no automatic deletion.
- [x] Update authoritative technical sections and any affected decisions to remove ambiguity between replacement contracts and legacy profile CRUD, capture context, and API sections. Identify which previous tests still prove unchanged guarantees and which must be rewritten.
- [x] Review every [C0 acceptance condition](docs/implementation-plan.md#c0--complete-the-minimum-replacement-contract); record decisions and evidence before editing implementation code.

## C1 — Projects, templates, and independent running sessions

References: [C1 scope and acceptance](docs/implementation-plan.md#c1--projects-templates-and-independent-sessions). **Accepted:** [C1 evidence](docs/reviews/c1-acceptance.md), 43 passing tests including sandboxed runs; public IPC/CLI remain C3 work.

- [x] Replace the central profile registry/CRUD path with on-demand immediate-directory discovery; recognize an empty project without metadata and never watch or rewrite project definitions.
- [x] Implement safe, race-aware project creation from a selected template into the first free numbered directory; test failure during copy and containment without touching user projects.
- [x] Adapt session persistence and start to record only stable session identity, source names, numbering, lifecycle, and active role. Remove live profile referential guards, rename propagation, and frozen root/application snapshots from the new runtime contract; ignore old persisted state under the C0 clean-break policy without automatic deletion.
- [x] Implement core-only start/switch/stop and restart behavior, reusing proven numbering and storage primitives only after their invariants are checked against the new model.
- [x] Test project rename/removal/reuse without changing an open session; start under an absent old name fails, while switch/stop succeed without a directory. Test generated-name collision, atomic creation, and concurrent project creation in isolated roots.
- [x] Review C1 acceptance with temporary filesystem and host/home/display/adapters trapped; record evidence for the **new** contract, not merely old M1/M2 results.

## C2 — Current application lists

References: [C2 scope and acceptance](docs/implementation-plan.md#c2--current-application-list). **Accepted:** [C2 evidence](docs/reviews/c2-acceptance.md), 79 passing tests normally and sandboxed (36 new C2 fixtures); daemon/API/CLI integration remains C3 work.

- [x] Implement bounded Linux XDG desktop-entry discovery using explicit test XDG roots, without a launcher dependency or executing applications.
- [x] Apply the active project's current exclusion list, then local `.desktop` overrides, including the ability to reintroduce excluded IDs; return installed eligible applications when no session is active or its project directory is missing.
- [x] Resolve the recorded project name anew on every request. Test changes to installed/project files, directory removal and same-name replacement without daemon restart or persisted application snapshots.
- [x] Test XDG precedence/visibility, malformed/unreadable input under the C0 policy, result bounds, deterministic output, and zero access to host XDG directories in default tests; record C2 evidence.

## C3 — API, CLI, and initial release

References: [C3 scope and acceptance](docs/implementation-plan.md#c3--public-access-and-initial-release). **Accepted:** [C3 evidence](docs/reviews/c3-acceptance.md), 101 passing tests and fully isolated formatting/lint/build/docs/release checks. [Installation and operation](docs/usage.md). C0–C3 initial-release work is complete; no live deployment was performed.

- [x] Provide the C0 versioned owner-only local API and matching CLI for C1/C2 behaviors; do not include profile/task CRUD or adapter lifecycle endpoints as initial-release prerequisites.
- [x] Test full project-create → session-start → app-list → switch/stop flows, no-active/missing-project fallback, restart, concurrent and malformed requests, bounded replies, and the C0 clean-break/version policy using temporary sockets/config/state/XDG roots.
- [x] Document installation, the incompatible upgrade and unsupported session rollback, and one generic launcher-client example. Do not require Hyprland, a browser, or a particular launcher for release.
- [x] Run formatting, linting, build, docs, and isolated tests with host desktop, installed adapters, home, and XDG access trapped and network disabled; evaluate and record C3 acceptance. Release may proceed without L1 or L2.

## Later, independently scheduled

### L1 — Optional task presets

- [ ] Specify project-local task file format and current application subset rules; settle task discovery/start API, versioning, name changes, missing project behavior, and collisions before implementation. Until then, ignore `.session-manager/tasks/` files in the initial product.
- [ ] Add task discovery and independently numbered task sessions without changing project/session independence or requiring pause/resume; test against current app lists and evaluate [L1](docs/implementation-plan.md#l1--tasks-optional-later-delivery).

### L2 — Optional pause/resume

- [ ] Revise the generic capture context and API for name-only sessions without a frozen project snapshot; specify the new checkpoint contract before enabling adapters; old checkpoint compatibility is not required.
- [ ] Reuse or adapt previously tested opaque storage **only after** confirming its runtime references, integrity and privacy guarantees against the revised contract; implement safe adapter invocation, capture/restore/discard, truthful uncertain outcomes and recovery with isolated fakes and conformance tests.
- [ ] Evaluate [L2](docs/implementation-plan.md#l2--pauseresume-optional-later-delivery) separately; never make it a hidden C3 dependency or validate it against the live desktop without explicit approval.
