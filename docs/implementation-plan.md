# Implementation Plan

## Purpose and status

This is the **current** staged roadmap for the [functional specification](functional-specification.md), the [replacement contract in the technical specification](technical-specification.md#replacement-contract-directory-backed-projects-and-application-resolution), and [ADR 0012](decisions/0012-discover-project-profiles-and-resolve-xdg-applications.md). The [previous roadmap](legacy-implementation-plan.md) and M0–M2 acceptance records below document work against central TOML profiles; they are not acceptance of the new model. Reuse tested code only where its invariants still apply. Do not implement the legacy M3–M6 sequence by default.

The initial useful product is small: discover projects, create one from a template, manage running sessions, and answer current application-list queries. Tasks are a separate increment; pause/resume and checkpoint adapters are another. Neither later feature blocks the initial release. No implementation milestone starts until its required technical contract is settled.

## Delivery rules

- A project directory is the current source of truth. Do not mirror profile files or app lists into session state, add filesystem watching, or maintain a live project-to-session foreign key.
- Session start records a UUID, recorded source name, number, and active role in one durable commit. Do not add profile CRUD, rename propagation, deletion guards, or a frozen app/root snapshot.
- Application queries use XDG entries plus current project exclusions and local overlays; no launcher, Firefox, compositor, or app-launch logic belongs in the core.
- Keep the existing Rust package and safe temporary-root testing approach when compatible. M0–M2 modules and tests are a migration starting point, not a reason to preserve obsolete contracts or premature adapter complexity in the first release.
- Implement in small reviewable slices: settle a contract, update its owning technical specification, implement, test through public behavior, then record evidence. Never test against the live desktop or host XDG directories by default.

## Milestones

| Stage | Outcome | Gate |
| --- | --- | --- |
| C0 | Replacement contracts and clean-break state policy | Before changing implementation |
| C1 | Directory-backed projects, templates, and running sessions | C0 project/runtime contracts |
| C2 | On-demand XDG application listing | C0 application contract; C1 |
| C3 | Versioned local API, CLI, integration tests, initial release | C1–C2 and C0 API contract |
| L1 (later) | Optional task presets | C3; task-file/API contract |
| L2 (later) | Optional pause/resume and adapter checkpoints | C3; revised adapter/capture and recovery contract |

### C0 — Complete the minimum replacement contract

**Goal:** remove ambiguity before code, without turning deferred features into initial-release requirements.

- Specify the single project-source setting, name and path validation, template selection and copy safety, collisions/concurrent creation, bounds, symlink handling, and how incomplete creations are recovered or cleaned up.
- Specify the exclusion file's name, format, duplicate/invalid-entry behavior, desktop-entry identity, XDG directory precedence and visibility, local override validation, failure behavior, and bounded API result. Read installed entries only; listing never executes a desktop entry.
- Specify a new public API version and its minimum methods/results for project list/create, template list, session start/list/switch/stop, and current application list, including no-active and missing-project behavior. Keep old version 1 wire shapes immutable; decide explicitly whether to serve them concurrently, reject them, or migrate users.
- Specify the new runtime shape without central profile foreign keys or snapshots, the treatment of existing config/runtime/checkpoints and rollback. C0 accepts a clean break: ignore legacy artifacts without migration, recovery, startup gates, or continued usability; do not automatically delete them. Decide necessary revisions/events and durable writes for the minimal product; do not transplant the old adapter registry, ETags, and subscription machinery without a real initial requirement.
- Document the boundary: core Linux XDG discovery versus downstream launcher/desktop presentation. Treat the old technical specification's profile CRUD, immutable start snapshot, rename transactions, and version 1 IPC as **legacy**, not constraints to implement.

**Acceptance:** the updated technical specification and relevant decisions unambiguously describe the initial version's files, persistence, errors, API and clean-break/version policy; no unresolved item above is guessed in implementation. The historical M0–M2 acceptance records remain unchanged.

### C1 — Projects, templates, and independent sessions

**Goal:** make the filesystem authoritative and core-only session lifecycle usable without an adapter or launcher.

- Read immediate project subdirectories on demand; absence of project metadata is valid. Create a project only by selecting a template and reserving the first free `Name-N` target; copy without merging, overwriting, escaping roots, or leaving an ambiguous half-project.
- Build on existing tested numbering and UUID behavior where applicable. Start verifies the requested project exists, commits its recorded name and generated session identity atomically, and makes it active. Switch selects another running session; stop drops only logical runtime state. Neither requires its project's directory after start.
- Handle manual rename/deletion without propagating changes. A newly created directory with an old name supplies later contextual lookups; existing session identity remains unchanged. Reject generated-name collisions rather than inventing a profile linkage.
- Keep first-stage runtime storage minimal and crash-safe; do not make task parsing, checkpoint creation, adapter startup, or desktop access a dependency.

**Acceptance:** isolated filesystem fixtures cover empty folders, template selection, first-free numbering, collisions, malformed/unsafe templates, failure mid-copy, concurrent create attempts, sequential session numbering, restart, manual project rename/removal/replacement, start rejection of missing projects, and switch/stop with missing projects. Tests prove no persistent project snapshot, rename cascade, adapter effect, host filesystem write, or live-desktop access.

### C2 — Current application list

**Goal:** return launcher-independent app data reflecting the machine and project at query time.

- Resolve installed desktop entries following the C0 XDG contract using injected temporary XDG roots, not the host application directories in tests.
- Remove excluded desktop-entry IDs and overlay project-local `.desktop` entries last, including reintroduction after exclusion; validate/bound untrusted files without executing them.
- Resolve the active session's recorded project name anew on each request; when no session is active or the directory is absent, return eligible installed apps only. When a different directory takes that name, use its current settings.
- Expose the result via the C0 API for any launcher client, with no baked-in browser profile, launcher command, app execution, or stored application snapshot.

**Acceptance:** fixtures prove XDG precedence/visibility and duplicate handling, local overrides and reintroduction, missing/invalid local files under the documented error policy, repeated queries reflecting edits without a daemon restart, no-active/missing/reused-name fallback, deterministic bounded results, and zero dependence on the user's installed apps or graphical session.

### C3 — Public access and initial release

**Goal:** provide a complete small service through the new versioned API and CLI.

- Expose the C0 methods through an owner-only local daemon/API and matching CLI. Apply only preconditions and diagnostics actually specified for these operations; avoid profile/task CRUD and optional adapter lifecycle endpoints.
- Test project creation, listing, session lifecycle, and application-list requests end to end with temporary config, state, XDG directories, and sockets; test restart, races, malformed input, bounded replies, and the C0 clean-break/version policy.
- Document install, upgrade/rollback, safety boundaries, and a minimal external launcher-client example that requests apps and passes the result to a launcher. Do not require a specific compositor or launcher for release.

**Acceptance:** a fresh isolated installation can create a project from either the default or a custom template, start/switch/stop sessions, and obtain up-to-date apps without a desktop. API/CLI behavior agrees; no rejected request partially mutates state; existing user state is not silently discarded or rewritten. Tests pass with host/home/display/adapters trapped and network disabled.

### L1 — Tasks (optional later delivery)

Keep named project-local task presets as ordered application-ID subsets, with independently numbered task sessions. First specify file format, discovery/start API, validation against the **current** project app list, rename/removal behavior, and compatibility/versioning. Then implement and test them without changing C1–C3 project semantics. Until L1 ships, task files are ignored, not errors.

### L2 — Pause/resume (optional later delivery)

Before implementing, reconcile the old adapter/checkpoint contract with name-only project context and the absence of a start snapshot. Specify capture inputs, compatible restore and discard, durable recovery, API version, and new checkpoint persistence. Compatibility with old stored checkpoints is not required. Preserve the generic principle: opaque payloads are never reconstructed from project files, uncertain effects are not reported as completed, and default tests cannot touch the live desktop. Implement and accept the adapter protocol, fake/conformance evidence, lifecycle recovery, and privacy guarantees in this stage rather than requiring them in C1–C3.

## Trace and release gates

| Functional behavior | Stage | Evidence |
| --- | --- | --- |
| Project discovery and templates | C0, C1, C3 | Temporary directories, safe-copy/race and API fixtures |
| Name-only sessions, numbering, active context | C0, C1, C3 | Pure naming tests, restart and rename/removal/reuse fixtures |
| Live XDG and local app list with fallback | C0, C2, C3 | Injected XDG roots and API end-to-end tests |
| Launcher/compositor independence and default test safety | C1–C3 | Negative schema/dependency tests and host traps |
| Task presets | L1 only | Project-local task and subset fixtures |
| Adapter-owned checkpoints and truthful recovery | L2 only | Isolated adapter conformance and crash/byte-preservation tests |

C3 may be accepted without L1 or L2. Each stage has its own evidence record; no prior acceptance record proves a later stage. If a required technical decision remains open, resolve it in the specification before marking the affected stage ready.

## C0 acceptance record

```text
Milestone: C0
Source revision: working tree based on d7318685cda07b34f56af7f50aadc6ce0bf17d59
Status: accepted
Evaluated at: 2026-11-04
Evidence: owner approved policies 2–4 and removed legacy functionality/session/checkpoint usability requirements; docs/reviews/c0-contract-review.md contains the gate matrix and static M1/M2 reuse audit. Documentation-only changes; no implementation or desktop tests run.
Specification trace: technical specification, Initial-release technical contract (version 2); ADR 0013
Open conditions: none for C0; C1–C3 implementation and acceptance remain outstanding
```

## C1 acceptance record

```text
Milestone: C1
Source revision: working tree based on d7318685cda07b34f56af7f50aadc6ce0bf17d59
Status: accepted
Evaluated at: 2026-11-05
Evidence: RUSTUP_TOOLCHAIN=1.96.1 CARGO_NET_OFFLINE=true make check; RUSTUP_TOOLCHAIN=1.96.1 make test-isolated; git diff --check (all passed). 43 tests pass normally and with host home/session/application roots hidden, a cleared environment and network/PID isolation. Detailed trace: docs/reviews/c1-acceptance.md.
Specification trace: C1 scope and acceptance; technical specification Settings and project lookup, Templates and project creation, Runtime and compatibility; ADRs 0012/0013
Open conditions: none for C1; C2 application resolution and C3 daemon/API/CLI remain outstanding
```

## C2 acceptance record

```text
Milestone: C2
Source revision: working tree based on d7318685cda07b34f56af7f50aadc6ce0bf17d59
Status: accepted
Evaluated at: 2026-11-05
Evidence: RUSTUP_TOOLCHAIN=1.96.1 CARGO_NET_OFFLINE=true make check; RUSTUP_TOOLCHAIN=1.96.1 make test-isolated; git diff --check (all passed). 79 tests pass normally and with hidden host home/session/application roots, a cleared environment and network/PID isolation. 36 new C2 fixtures cover current queries, strict XDG parsing/precedence/visibility, overlays/fallback, bounds and metadata-only TryExec probes. Detailed trace: docs/reviews/c2-acceptance.md.
Specification trace: C2 scope and acceptance; technical specification Application resolution and Resolution details and query consistency; ADR 0012
Open conditions: none for C2; C3 environment/bootstrap, daemon/API/CLI and release acceptance remain outstanding
```

## C3 acceptance record

```text
Milestone: C3
Source revision: working tree based on d7318685cda07b34f56af7f50aadc6ce0bf17d59
Status: accepted for initial release scope
Evaluated at: 2026-11-05
Evidence: RUSTUP_TOOLCHAIN=1.96.1 CARGO_NET_OFFLINE=true make check; RUSTUP_TOOLCHAIN=1.96.1 make check-isolated; bash -n tools/test-isolated.sh tools/check-isolated.sh; git diff --check (all passed). 101 tests (94 library and 7 executable integration) pass normally and in the full PID/network-isolated build/check environment. The isolated target also builds the release binary offline with --locked and checks version output. Detailed trace: docs/reviews/c3-acceptance.md.
Specification trace: C3 scope and acceptance; technical specification version 2 public API/startup/transport; usage guide; ADRs 0003/0013
Open conditions: none for C3; L1/L2 remain optional later deliveries. No live deployment or package publication was performed.
```

## Historical acceptance records

The following are retained verbatim from the old plan. Their M0–M2 references and acceptance criteria refer to the [historical roadmap](legacy-implementation-plan.md), not stages C0–L2.

Each historical milestone evaluation appended:

```text
Milestone: Mx
Source revision: <commit>
Status: accepted | rejected
Evaluated at: <UTC timestamp>
Evidence: <commands and sanitized result links>
Specification trace: <section/test references>
Open conditions: <none for accepted milestones>
```

A rejected record remains historical. A later evaluation appends a new record.

```text
Milestone: M0
Source revision: uncommitted working tree based on 8471ab1eabf296b4ec222e7fe2631716eff257f5
Status: accepted
Evaluated at: 2026-07-17T16:18:43Z
Evidence: RUSTUP_TOOLCHAIN=stable CARGO_NET_OFFLINE=true cargo fmt --all -- --check; cargo test --all-targets --all-features (6 passed); cargo clippy --all-targets --all-features -- -D warnings; RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features; cargo build --all-targets --all-features (all passed)
Specification trace: M0 scope and M0-A1--A4; technical specification, Testing strategy; ADR 0001, ADR 0009
Open conditions: none
```

```text
Milestone: M1
Source revision: uncommitted working tree based on 8471ab1eabf296b4ec222e7fe2631716eff257f5
Status: accepted
Evaluated at: 2026-07-17T17:03:13Z
Evidence: RUSTUP_TOOLCHAIN=stable CARGO_NET_OFFLINE=true cargo fmt --all -- --check; cargo build --all-targets --all-features; cargo test --all-targets --all-features (28 passed); cargo clippy --all-targets --all-features -- -D warnings; RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features (all passed)
Specification trace: M1 scope and M1-A1--A8; functional specification, technical specification Configuration, Profile and Session sections; ADR 0002 and ADR 0010
Open conditions: none
```

```text
Milestone: M2
Source revision: uncommitted working tree based on d7318685cda07b34f56af7f50aadc6ce0bf17d59
Status: accepted
Evaluated at: 2026-07-22T12:32:00Z
Evidence: RUSTUP_TOOLCHAIN=stable CARGO_NET_OFFLINE=true cargo fmt --all -- --check; cargo build --all-targets --all-features; cargo test --all-targets --all-features (64 passed); cargo clippy --all-targets --all-features -- -D warnings; RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features (all passed). Transaction fixtures cover every candidate, synchronization, manifest, replacement, runtime-reference, and cleanup interruption boundary.
Specification trace: M2 scope and M2-A1--A8; technical specification Runtime metadata, Checkpoint envelope, Opaque payload invariant, and Atomicity; ADR 0010
Open conditions: none. Direct adapter compatibility and descriptor transport remain separately owned and tested by M3/M4.
```

```text
Milestone: M2
Source revision: uncommitted working tree based on d7318685cda07b34f56af7f50aadc6ce0bf17d59
Status: accepted (supersedes the preceding M2 record)
Evaluated at: 2026-07-22T12:53:30Z
Evidence: RUSTUP_TOOLCHAIN=stable CARGO_NET_OFFLINE=true cargo fmt --all -- --check; cargo build --all-targets --all-features; cargo test --all-targets --all-features (69 passed); cargo clippy --all-targets --all-features -- -D warnings; RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features (all passed). Review-remediation fixtures cover candidate-directory synchronization, pair-preserving quarantine/deletion, bounded sparse-file reads, creation-set recovery, revision exhaustion, and operation timestamp ordering.
Specification trace: M2 scope and M2-A1--A8; technical specification Persistent runtime and checkpoint storage, Runtime metadata, Checkpoint envelope, Opaque payload invariant, and Atomicity; ADR 0002 and ADR 0010
Open conditions: none. Direct adapter compatibility and descriptor transport remain separately owned and tested by M3/M4.
```

## Plan maintenance

Keep implementation work within its stage and record evidence only after its acceptance checks pass. A contract change updates the functional/technical specifications and relevant ADR before code or roadmap changes. Preserve historical evidence; do not transfer acceptance from the central-profile design to the replacement. Concrete desktop clients and adapters remain separate deliverables, not dependencies of the initial release.
