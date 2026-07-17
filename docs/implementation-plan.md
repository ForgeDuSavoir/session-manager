# Implementation Plan

## Purpose and status

This plan orders implementation of the platform-independent Session Manager core defined by the [functional specification](functional-specification.md), [technical specification](technical-specification.md), and [ADR 0010](decisions/0010-keep-the-core-platform-independent-with-opaque-adapter-checkpoints.md).

The plan is ready for implementation. The profile/task schema, adapter lifecycle vocabulary, checkpoint envelope, subprocess transport, lifecycle transitions, and integration scope are fixed in the specifications.

## Delivery principles

1. Keep platform-specific types and dependencies outside the core package and public API.
2. Establish safe tests and dependency guards before implementing stateful behavior.
3. Implement domain and persistent data contracts before IPC or adapter invocation.
4. Prove opaque payload byte preservation before lifecycle composition.
5. Exercise all adapter behavior through a fake and reusable conformance suite.
6. Persist intent before non-atomic adapter requests and never replay uncertain non-idempotent work.
7. Make every milestone independently reviewable through public or contract-level evidence.

## Milestone overview

| Milestone | Outcome | Predecessors |
| --- | --- | --- |
| M0 | Safe core repository and dependency boundary | Design validation complete |
| M1 | Platform-independent domain and lossless configuration | M0 |
| M2 | Durable runtime, checkpoint, and transaction storage | M1 |
| M3 | Generic adapter contract, fake adapter, and conformance kit | M2 |
| M4 | Lifecycle service and conservative recovery | M2, M3 |
| M5 | Generic local API and CLI | M4 |
| M6 | Core integration hardening and standalone release | M5 |

## M0 — Safe core foundation

**Goal:** create a buildable Rust core whose default tests cannot reach a concrete adapter or live desktop.

**Scope:**

- create the Rust 2024 package, library, executable, formatting, linting, documentation, and CI configuration;
- create domain, application, storage-port, adapter-contract, and interface module boundaries;
- enforce dependency direction and prohibit platform-specific crates, modules, environment discovery, and production adapter construction in library tests;
- provide deterministic clock and UUID fakes, canonical temporary XDG roots, ownership sentinels, path containment, and installed-adapter/live-desktop trap endpoints;
- run default checks with network disabled.

### M0 acceptance

- **M0-A1:** a clean checkout passes formatting, linting, build, documentation, and all-target tests with the pinned stable Rust toolchain.
- **M0-A2:** architecture tests prove domain and application modules import no concrete adapter or platform-specific type.
- **M0-A3:** the default suite records zero attempts to discover adapters or contact display, compositor, user-bus, process-manager, or host XDG trap endpoints.
- **M0-A4:** temporary-root tests reject relative paths, the real home, `/`, symlink escapes, foreign sockets, missing sentinels, and unsafe cleanup.

## M1 — Domain and configuration

**Goal:** implement profiles, tasks, session identity, and lossless configuration without desktop dependencies.

**Scope:**

- implement name-identified profiles whose tasks contain only ordered desktop-entry subsets, plus launch-source, session/checkpoint UUID, session-as-workspace-group, and lifecycle values;
- implement pure naming and numbering from a supplied live-session set, plus rename propagation, collision validation, and deletion guards without a persisted pre-commit reservation; lifecycle composition owns the later atomic start commit;
- implement strict TOML decoding and complete semantic validation;
- retain accepted bytes and implement smallest-boundary format-preserving CRUD;
- implement configuration ETags, on-disk identity checks, explicit reload, and offline validation;
- add property, state-machine, hostile-input, and noncanonical-format fixtures.

### M1 acceptance

- **M1-A1:** profiles and tasks within every documented file, count, name, root, and desktop-entry bound decode; an out-of-bound value, non-lexically-normal root, task entry outside its parent profile, or forbidden non-core field fails with a stable path.
- **M1-A2:** property sequences preserve lifecycle-independent identity, numbering, rename, and deletion invariants.
- **M1-A3:** the documented FDS/QMK numbering examples pass for sequential starts, including stopped-number reuse, delimiter-derived cross-source name collision rejection, no collision-driven number skipping, and no numbering effect from uncommitted transaction candidates.
- **M1-A4:** profile and task rename update all related running/paused names while preserving session/checkpoint UUIDs, numbers, source association, lifecycle, and immutable source snapshots; M4 supplies the later storage/adapter integration proof.
- **M1-A5:** every supported CRUD operation preserves all unrelated TOML bytes, comments, whitespace, quoting, and ordering.
- **M1-A6:** an external file change produces `conflict.configuration_changed_on_disk` without writes until explicit reload accepts it.
- **M1-A7:** offline validation reads only the explicit file and creates no daemon, state, socket, or adapter access.
- **M1-A8:** pure reload-policy and configuration-storage tests accept additions, non-identity edits, and unreferenced removals atomically, but reject apparent rename/removal against a supplied live-reference snapshot, collisions, and concurrent configuration-file changes without partial adoption; M4 supplies joint runtime-precondition/locking evidence.

## M2 — Runtime and opaque checkpoint storage

**Goal:** provide crash-safe core state and byte-exact checkpoint persistence before invoking adapters.

**Scope:**

- implement strict runtime JSON, revisions, shutdown markers, attention, pending operations, and previous validated state;
- implement transport-neutral persisted adapter IDs, format IDs, protocol versions, exact public session-operation kinds/phases/outcomes/timestamps, pending lifecycle intents with handoff/exit proof, adapter-release obligations, historical correlation UUID rules, bounded public operation history, and configuration/runtime projection-capacity components later composed by M4;
- implement durable monotonic retention time through `max_observed_unix_ms`, including backward and forward wall-clock jumps;
- persist the core operation UUID and complete immutable adapter intent before invocation so recovery never depends on an acceptance token;
- implement the exact version 1 checkpoint envelope, 64 MiB payload limit, SHA-256 integrity, confidentiality, reference, retention, unsupported-format, and deletion rules;
- store payloads atomically as opaque bytes under UUID-derived names;
- implement configuration/runtime/checkpoint transaction manifests and complete-set recovery;
- implement immutable payload handoff and explicit rejection of unsupported format versions without migration;
- add deterministic fault points at every file, directory, manifest, reference, and cleanup boundary.

### M2 acceptance

- **M2-A1:** runtime fixtures reject invalid live references without rewriting them while accepting terminal-operation and release-obligation historical session UUIDs after their live session/checkpoint has been removed.
- **M2-A2:** arbitrary binary payload fixtures, including empty, non-UTF-8, and boundary-sized values, round-trip byte-for-byte across write, restart, handoff, and backup selection.
- **M2-A3:** digest, length, UUID, machine identity, unsupported envelope version, path traversal, and relabel attempts fail before adapter handoff.
- **M2-A4:** fault injection always leaves one complete validated old or new configuration/runtime/checkpoint set and never reconstructs TOML or payload bytes.
- **M2-A5:** payload deletion occurs only after no committed state references it; uncertain and quarantined files are never silently adopted.
- **M2-A6:** logs, errors, runtime inspection, and fixtures expose no payload bytes or secrets.
- **M2-A7:** pending lifecycle intents, durable exact-child exit proof, and release obligations survive every restart boundary independently of the 30-day/1,000-record terminal public history policy; worst-case storage and configuration/runtime projection-component capacity is reserved before effects, and over-bound external state is retained read-only without dropping evidence.
- **M2-A8:** deterministic clock tests prove persisted `max_observed_unix_ms` survives restart, backward wall-clock changes never extend retention, forward jumps may expire eligible history, and operation ordering remains revision-based.

## M3 — Generic adapter contract and conformance kit

**Goal:** freeze and test the platform-neutral adapter boundary independently of lifecycle composition.

**Entry condition:** M2 is accepted; the completed-capture reconciliation, adapter-registry lifecycle, and operation-status retention contracts are normative in the technical specification and ADR 0011.

**Scope:**

- implement capability IDs and capability-qualified format ranges, then subprocess DTOs and codecs over the M2 transport-neutral identifiers and persisted operation enums, including every method-specific request/result, accepted/final envelope, stable protocol error, deadline, cancellation, idempotency, and operation-UUID status-recovery rule;
- implement the explicit owner-only adapter registry with accepted bytes, file identity, source ETag, separate adapter-generation UUID, explicit atomic reload, pending-operation guards, and canonical executable, ownership, permission, identity, and environment validation;
- implement the adapter-registry public projection-capacity component for later exact composition by M4;
- implement race-free descriptor-based execution of the exact opened and validated adapter image for every spawn, with deterministic accepted hello metadata cached by file identity plus accepted adapter generation and uncached-image hello plus requested invocation bound to duplicate descriptors of that image;
- implement bounded one-request JSON Lines subprocess framing from ADR 0011 behind the typed application port;
- implement inherited descriptor mapping and byte-exact capture output plus restore/discard input transfer;
- implement protocol codecs and transport validation for pre-handoff failure, acceptance-unknown, permitted pre-acceptance finals, and accepted-then-final sequences; after complete request handoff use no termination signal and put adapter-owned operation persistence, status retention, release, and completed-capture re-export behavior in the sample adapter and conformance fixtures rather than the core transport;
- implement the distinct application fake port and fake subprocess adapter, covering every capability, outcome, concurrency, timeout, disconnect, and recovery combination at their respective boundaries;
- build a standalone sample-adapter fixture outside the production Cargo package that imports no core internals and is not shipped as part of the core artifact;
- publish reusable adapter-conformance fixtures for byte-exact payload handoff and re-export, version mismatch, capability gating, registry reload, timeouts, cancellation, crash/reconnect, operation status/release, concurrency, and privacy;
- ensure default composition selects only explicit fake/test adapters while production discovery remains isolated.

### M3 acceptance

- **M3-A1:** the fake subprocess adapter passes every finalized capability-qualified format, deterministic adapter-selected capture format, value/collection bound, and lifecycle wire-contract case without platform-specific values, while the application fake port independently covers typed-port outcomes without wire behavior.
- **M3-A2:** adapter identity and compatibility tests reject fuzzy, relabeled, missing, duplicate, incompatible, and downgraded matches before mutation.
- **M3-A3:** explicit-registry tests prove startup loading, distinct source-ETag and adapter-generation preconditions, a fresh startup generation, generation/event changes for accepted source or public-projection changes, exact source-and-projection no-op behavior, whole-registry validation, explicit reload only, immutable invocation registration generations, and rejection of changes/removal affecting any pending lifecycle intent, accepted or indeterminate operation, or release obligation; subprocess tests reject unsafe paths, ownership, permissions, IDs, and environment names, bind cached deterministic hello metadata to exact file identity plus accepted adapter generation, reject an uncached identity/generation pair whose public hello metadata differs until explicit reload, run uncached-image hello and the requested method from duplicate descriptors of one opened inode despite concurrent path replacement, leave public availability unchanged after transient invocation failure, and use exact JSON control schemas and inherited descriptors to preserve arbitrary payload bytes and correlate operations by durable core UUID.
- **M3-A4:** unsupported capabilities gate only dependent operations and do not prevent configuration or safe inspection; capability IDs and format IDs are unique in their owning lists, each format ID has one valid inclusive range per capability, and pause preflight/proposal validation require capture/restore compatibility plus discard compatibility when declared required, so no initially unrestorable checkpoint commits.
- **M3-A5:** the protocol distinguishes pre-handoff failure, post-handoff acceptance-unknown, accepted, indeterminate, and final sequences; after complete handoff no signal is sent, capture re-export status uses the mutation deadline, and absent status proves non-acceptance only after durable exact-child exit proof created by the spawning daemon.
- **M3-A6:** the independently built, non-production sample-adapter fixture imports no Session Manager internals, passes the reusable conformance kit, persists accepted and terminal records until explicit release, persists terminal results before final output, survives a compatible atomic registered-path replacement between invocations, and re-exports completed captures byte-for-byte after simulated transport loss.
- **M3-A7:** default tests cannot discover or invoke an installed concrete adapter.

## M4 — Lifecycle and recovery service

**Goal:** compose domain, persistence, and adapter behavior into truthful start, switch, pause, resume, stop, and recovery operations.

**Scope:**

- implement serialized overlapping mutations and concurrent reads;
- implement one explicit lifecycle-admission boundary after request, precondition, reference, lifecycle, naming, operation-specific adapter preflight, and capacity validation, creating no public operation for a rejected request and atomically creating operation identity plus recovery authority for an admitted request; core-only start, switch, running stop, and local paused stop require no adapter;
- implement core-only start with one locked atomic allocation/session/direct-final-operation commit and immutable source snapshot, and core-only logical switch;
- implement pause capture and atomic paused commit only after capture `completed`;
- implement resume compatibility, exact payload handoff, adapter verification, and running commit;
- implement stop cleanup/discard, checkpoint retention on uncertainty, final deletion, and number reuse;
- persist typed intent before adapter effects and reconcile status without blind replay;
- for every adapter operation known to have reached acceptance, atomically replace the lifecycle intent with a durable release obligation in its terminal core commit, including terminal outcomes that do not change session lifecycle; create no obligation for core-only or pre-acceptance outcomes, and remove an obligation only after a later completed idempotent release;
- implement durable attention plus automatic safe status/release reconciliation with no force or manual-resolution action.

### M4 acceptance

- **M4-A1:** every valid and invalid lifecycle transition passes through public application operations against the application fake port and matches the finalized transition table; validation, stale-precondition, missing-reference, lifecycle, naming, operation-specific unavailable/incompatible-adapter, and capacity rejections create no public operation or lifecycle event, while core-only operations require no adapter and every admitted request atomically commits its public operation with either its pending recovery authority or its core-only final state change.
- **M4-A2:** repeated sequential starts from one source each atomically produce a unique UUID, the documented highest-live-plus-one number, name, source snapshot, and direct-final operation while invoking zero adapter or platform operations and persisting no intermediate reservation; lower live-number gaps are not reused while a higher live number remains, and a collision aborts the whole start without exposing partial state.
- **M4-A3:** pause commits only after capture `completed` with a valid capability-related proposal and durable checkpoint; terminal capture failure or invalid accepted completion remains running, every accepted terminal result creates a release obligation, and acceptance-unknown or accepted-indeterminate capture remains running with recovery required while preserving no partial candidate as authoritative.
- **M4-A4:** resume sends exact stored bytes only to a compatible adapter; completed restore becomes running and active, completed-with-limitations becomes running and active with attention, all known non-completions remain paused with release obligations only after acceptance, and acceptance-unknown or accepted-indeterminate restore remains paused and recovery-required.
- **M4-A5:** running stop is core-only; paused stop deletes locally when adapter discard is not required and otherwise requires discard `completed`; every other required-discard result retains the paused checkpoint and number.
- **M4-A6:** daemon termination at every lifecycle boundary converges without replaying or signalling a handed-off non-idempotent adapter request; restart without exact-child exit proof retains acceptance-unknown.
- **M4-A7:** switch changes only generic active context, and clients receive one coherent core revision.
- **M4-A8:** missing, incompatible, unsupported, failed, cancelled, safely timed-out, acceptance-unknown, and indeterminate adapter cases remain distinguishable in lifecycle and attention results.
- **M4-A9:** every terminal commit for an accepted adapter operation atomically creates a release obligation, including accepted failure without lifecycle change; core-only and pre-acceptance outcomes create none, every crash before completed release retries idempotently, and only a later durable commit removes the obligation.
- **M4-A10:** rename integration tests preserve adapter identity, checkpoint envelope, and byte-exact payload while atomically updating every affected pending/live label across configuration and runtime.
- **M4-A11:** configuration reload validates its expected runtime revision and live-reference guards under the mutation/joint commit lock, including start-versus-reload, stop-versus-reload, rename-versus-reload, and repeated-reload ordering races.
- **M4-A12:** every application mutation validates the resulting combined snapshot and maximum commit event through the shared exact projection encoder before commit, and recovery gates externally assembled over-bound combinations read-only before publication, so no accepted writable state exceeds the public 16 MiB line bound.

## M5 — Local daemon, IPC, and CLI

**Goal:** expose the complete generic core through stable local interfaces usable by arbitrary integrations.

**Scope:**

- implement single-instance foreground daemon, owner-only Unix socket, and same-user peer checks;
- implement exact request/response/error/public-operation framing, handshake and version negotiation, request-ID validation, canonical compact JSON encoding, the 16 MiB combined snapshot/event bound, and source-document/collection bounds;
- implement stable error-family mapping and exact configuration-ETag/runtime-revision preconditions, including joint preconditions for reload, rename, and deletion guards;
- implement cursor pagination, stable collection ordering, limit enforcement, and stale-cursor rejection;
- implement configuration, profile, and task method schemas and handlers;
- implement session lifecycle method schemas and handlers;
- implement adapter list/get/compatibility/reload, operation, and attention method schemas and handlers, including source-ETag and adapter-generation preconditions;
- implement parameterless unconditional snapshot-first subscriptions on dedicated server-stream-only connections, with queue registration, coherent tuple capture, and first-snapshot enqueue atomic under the publication lock; implement atomic multi-projection commit events whose runtime replacement always carries `runtime.changed`, normative message/byte queue bounds, monotonic write-stall closure, frame-safe best-effort overflow resynchronization, reconnect, and shutdown publication;
- implement matching CLI commands, stable JSON output, human output, exact exit codes, and timeout behavior;
- expose safe checkpoint envelope metadata but no payload-content inspection or platform objects;
- add real temporary-socket daemon/CLI tests with the fake subprocess adapter and restart coverage.

### M5 acceptance

- **M5-A1:** protocol tests cover fragmented/coalesced frames, malformed input, duplicate request IDs, unknown methods/fields/enums/events, incompatible versions, handshake ordering, canonical compact encoding, the exact 16 MiB combined snapshot/event bound including LF, maximum valid projections, oversized messages, and slow clients.
- **M5-A2:** every generic CRUD and lifecycle command has matching IPC tests for success, validation, not found, exact operation-specific stale preconditions, adapter failure, JSON, human output, and exit code; a delete request itself is confirmation and carries no force flag.
- **M5-A3:** parameterless subscriptions reject pipelined trailing bytes and always publish the exact acknowledgement and initial snapshot before events on a dedicated server-stream-only connection; subscription-race tests prove a commit is represented either in the atomically captured initial snapshot or in a later queued event, never lost between registration and capture; the queue bounds apply as soon as the snapshot is enqueued, atomic multi-projection events prevent mixed state, every operation/attention change also carries `runtime.changed`, both queue limits trigger closure, five-second no-progress and 30-second total write deadlines cover the acknowledgement and every stream frame, and resynchronization is attempted only when no partial frame would be corrupted.
- **M5-A4:** public schemas contain no concrete platform names, window/process/layout fields, shortcut actions, or decoded payload content.
- **M5-A5:** clients never read persistent files directly and a handed-off acceptance-unknown or accepted operation continues without signal after client timeout or disconnect.
- **M5-A6:** downstream-client fixtures build a session list and lifecycle controls using only the generic API.
- **M5-A7:** real-socket tests remain inside temporary roots and invoke only the fake subprocess adapter.
- **M5-A8:** pagination tests prove limits, deterministic ordering, opaque cursors, and stale-cursor rejection, including adapter cursors invalidated by a generation change with an unchanged source ETag; projection tests prove every maximum valid source state fits without truncation.

## M6 — Core hardening and release

**Goal:** validate, package, and release the independent core and adapter contract.

**Scope:**

- complete the functional/technical/test trace and close uncovered cases;
- run clean default, parser/property, storage-fault, fake-adapter, conformance, IPC, and privacy suites;
- audit dependencies, public schemas, logs, fixtures, artifacts, and paths for platform coupling or private data;
- package the core daemon/CLI, example configuration, generic adapter contract/conformance kit, and service definition without a concrete adapter or shell integration;
- document installation, operation, adapter development, compatibility, recovery, upgrade, rollback, and uninstall;
- verify reproducible artifacts in a clean non-graphical environment.

### M6 acceptance

- **M6-A1:** every retained functional and technical requirement maps to passing automated evidence.
- **M6-A2:** dependency and public-schema audits find no concrete compositor, layout, shell, process manager, window model, or shortcut requirement.
- **M6-A3:** all default tests pass with graphical and installed-adapter traps and network disabled.
- **M6-A4:** a separately built sample adapter passes the published conformance kit and can be added or removed without rebuilding the core.
- **M6-A5:** privacy review finds no checkpoint payload, credential, environment secret, or machine-specific private data in logs or artifacts.
- **M6-A6:** install, upgrade, rollback, and uninstall preserve user configuration and retained checkpoints unless an explicit documented data action is requested.
- **M6-A7:** reproducible release artifacts contain no concrete platform integration.

## External integrations

Concrete adapters and clients are independent deliverables. They may be developed after the generic contract is accepted and may live in separate repositories. They do not gate the core MVP or first release. Their tests, compatibility matrices, installation, upgrades, and platform safety belong to their owning projects.

A downstream installer may compose a pinned Session Manager core, separately selected adapters, and client configuration only after tagged releases are available. Such composition remains outside this repository and cannot make integrations core dependencies.

## Design validation evidence

This section records the design-level closure performed on 2026-07-17 before M0. Implementation acceptance still requires the executable evidence in each milestone.

### Dependency-direction matrix

| Component | May depend on | Must not depend on | Enforced by |
| --- | --- | --- | --- |
| Domain | Rust standard/core abstractions and domain value types | Filesystem, IPC, adapter transport, concrete platform, client | M0-A2 architecture test |
| Application services | Domain plus typed storage, clock, UUID, publication, and adapter ports | Concrete adapter executable/protocol implementation details, compositor, shell, layout, process manager | M0-A2, M4 fake-port tests |
| Core storage | Core persistence types and filesystem port implementation | Adapter payload schema or external desktop state | M2-A2–A8 |
| Adapter contract | Platform-neutral IDs, envelopes, capabilities, requests, and outcomes | Concrete platform types, commands, windows, workspaces, layouts, processes | M3-A1, M5-A4 |
| Subprocess adapter transport | Generic adapter contract, explicit registry, bounded process/FD primitives | In-process plugin ABI, PATH/directory discovery, payload semantics, concrete adapter package | M3-A3, M3-A7 |
| IPC and CLI | Application services and public projection types | Client-specific models, compositor objects, decoded payloads | M5-A1–A8 |
| Concrete adapter | Published adapter protocol and its own platform dependencies | Core internals or private storage paths | M3-A6 conformance kit |
| Shell, launcher, compositor configuration, automation | Published generic public API | Core internals, checkpoint payload, authority to redefine core lifecycle | M5-A4, M5-A6 |

The only inward arrows are `interfaces -> application -> domain`, storage implementations toward storage ports, and subprocess transport toward the generic adapter port. Concrete adapters and clients are separately built consumers. No reverse import or packaging dependency is permitted.

### Functional-to-implementation trace

| Retained functional requirement | Technical contract | Acceptance evidence owner | Implementation tasks |
| --- | --- | --- | --- |
| Profile root and ordered desktop-entry CRUD | [configuration](technical-specification.md#configuration) | M1-A1, M1-A5–A8 | [M1](../ToDo.md#m1--domain-and-configuration) |
| Configuration lexical, count, and byte bounds | [configuration](technical-specification.md#configuration) | M1-A1, M1-A5–A8 | [M1](../ToDo.md#m1--domain-and-configuration) |
| Tasks are ordered parent-profile desktop-entry subsets with no placement fields | [profile model](technical-specification.md#profile) | M1-A1 | [M1](../ToDo.md#m1--domain-and-configuration) |
| Per-source generated names and stopped-number reuse | [naming transactions](technical-specification.md#naming-and-rename-transactions) | M1-A2–A4 | [M1](../ToDo.md#m1--domain-and-configuration) |
| Atomic rename propagation, guarded reload, and deletion guards | [configuration validation](technical-specification.md#validation-and-format-preservation) | M1-A4–A6, M1-A8, M4-A10–A11 | [M1](../ToDo.md#m1--domain-and-configuration), [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| One session is one platform-independent logical workspace group | [session model](technical-specification.md#session) | M1-A2, M4-A2 | [M1](../ToDo.md#m1--domain-and-configuration), [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| Running/paused lifecycle and separate active role | [lifecycle invariants](technical-specification.md#lifecycle-invariants) | M1-A2, M4-A1 | [M1](../ToDo.md#m1--domain-and-configuration), [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| Start and switch are core-only | [session operations](technical-specification.md#session-operations) | M4-A1–A2, M4-A7 | [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| Pause commits only a completed valid capture | [lifecycle transition mapping](technical-specification.md#session-operations) | M4-A3, M4-A8 | [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| Resume uses exact payload and truthful limited/failed outcomes | [lifecycle transition mapping](technical-specification.md#session-operations) | M4-A4, M4-A8 | [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| Running stop is core-only; paused stop respects discard requirement | [lifecycle transition mapping](technical-specification.md#session-operations) | M4-A5–A6 | [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| Opaque checkpoint integrity, confidentiality, retention, and mismatch rejection | [checkpoint envelope](technical-specification.md#checkpoint-envelope), [opaque invariant](technical-specification.md#opaque-payload-invariant) | M2-A2–A8 | [M2](../ToDo.md#m2--runtime-and-opaque-checkpoint-storage) |
| UUID/machine-ID separation, runtime capacity, monotonic retention time, and bounded terminal operation history | [runtime metadata](technical-specification.md#runtime-metadata), [checkpoint envelope](technical-specification.md#checkpoint-envelope) | M2-A1, M2-A3, M2-A7–A8 | [M2](../ToDo.md#m2--runtime-and-opaque-checkpoint-storage) |
| Exact public operation phases and historical target references | [public operation records](technical-specification.md#public-operation-records) | M2-A1, M4-A1, M4-A6, M5-A2 | [M2](../ToDo.md#m2--runtime-and-opaque-checkpoint-storage), [M4](../ToDo.md#m4--lifecycle-and-recovery-service), [M5](../ToDo.md#m5--local-daemon-ipc-and-cli) |
| Explicit lifecycle admission with no operation for synchronous rejection | [public operation records](technical-specification.md#public-operation-records) | M4-A1, M5-A2 | [M4](../ToDo.md#m4--lifecycle-and-recovery-service), [M5](../ToDo.md#m5--local-daemon-ipc-and-cli) |
| Explicit compatible subprocess adapter with capability-qualified formats | [generic adapter contract](technical-specification.md#generic-adapter-contract), [transport](technical-specification.md#transport-boundary) | M3-A1–A7 | [M3](../ToDo.md#m3--generic-adapter-contract-and-conformance-kit) |
| Race-free execution of the opened validated adapter image | [transport boundary](technical-specification.md#transport-boundary) | M3-A3 | [M3](../ToDo.md#m3--generic-adapter-contract-and-conformance-kit) |
| Explicit atomic adapter-registry reload | [transport boundary](technical-specification.md#transport-boundary), [public IPC](technical-specification.md#public-ipc-protocol-version-1) | M3-A3, M5-A1–A3 | [M3](../ToDo.md#m3--generic-adapter-contract-and-conformance-kit), [M5](../ToDo.md#m5--local-daemon-ipc-and-cli) |
| Conservative interruption, durable child-exit proof, capture re-export, release obligation, and idempotent release | [transport boundary](technical-specification.md#transport-boundary), [errors and recovery](technical-specification.md#errors-attention-and-recovery) | M2-A7, M3-A5–A6, M4-A3, M4-A6, M4-A8–A9 | [M2](../ToDo.md#m2--runtime-and-opaque-checkpoint-storage), [M3](../ToDo.md#m3--generic-adapter-contract-and-conformance-kit), [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| Generic bounded public projections for arbitrary clients | [public projections](technical-specification.md#public-projections), [CLI and IPC](technical-specification.md#cli-and-ipc) | M5-A1–A8 | [M5](../ToDo.md#m5--local-daemon-ipc-and-cli) |
| Exact public value and combined 16 MiB snapshot/event bounds | [runtime metadata](technical-specification.md#runtime-metadata), [public IPC](technical-specification.md#public-ipc-protocol-version-1) | M2-A7, M4-A12, M5-A1, M5-A8 | [M2](../ToDo.md#m2--runtime-and-opaque-checkpoint-storage), [M4](../ToDo.md#m4--lifecycle-and-recovery-service), [M5](../ToDo.md#m5--local-daemon-ipc-and-cli) |
| Canonical public encoding, exact subscription envelopes, runtime event qualifiers, and bounded overflow | [public IPC](technical-specification.md#public-ipc-protocol-version-1) | M4-A12, M5-A1, M5-A3, M5-A8 | [M4](../ToDo.md#m4--lifecycle-and-recovery-service), [M5](../ToDo.md#m5--local-daemon-ipc-and-cli) |
| Deterministic automatic reconciliation backoff | [errors and recovery](technical-specification.md#errors-attention-and-recovery) | M4-A6, M4-A8–A9 | [M4](../ToDo.md#m4--lifecycle-and-recovery-service) |
| No concrete desktop integration in core MVP or release | [design constraints](technical-specification.md#design-constraints), [external integrations](#external-integrations) | M6-A2, M6-A7 | [M6](../ToDo.md#m6--core-hardening-and-standalone-release) |
| Default tests cannot reach installed adapters or live desktop | [testing strategy](technical-specification.md#testing-strategy) | M0-A3–A4, M3-A7, M6-A3 | [M0](../ToDo.md#m0--safe-core-foundation), [M3](../ToDo.md#m3--generic-adapter-contract-and-conformance-kit), [M6](../ToDo.md#m6--core-hardening-and-standalone-release) |

Every included item in [First-version scope](functional-specification.md#first-version-scope) maps to at least one row. Explicit non-goals map to the dependency matrix, M5-A4 negative public-schema tests, M6-A2 dependency audit, and M6-A7 package audit.

### Opaque payload round-trip proof obligation

Let `B` be the exact byte sequence produced through the capture output descriptor, `L = len(B)`, and `H = SHA-256(B)`.

| Boundary | Core operation | Required invariant |
| --- | --- | --- |
| Capture receipt | Stream descriptor into same-directory candidate | Candidate bytes equal `B`; reject `L > 64 MiB` |
| Checkpoint commit | Synchronize payload, assemble envelope, commit reference | Envelope stores exactly `L` and `H`; payload is not parsed or transformed |
| Daemon restart | Reopen referenced payload read-only | Recomputed length/digest equal `L`/`H` before checkpoint becomes usable |
| API inspection | Project safe envelope metadata | No payload byte or decoded field enters the public model |
| Resume/discard handoff | Reopen verified payload on inherited read-only descriptor at offset zero | Adapter reads exactly `B`; it receives no storage path |
| Failure/recovery | Select a complete transaction set or request completed-capture re-export | Core uses an already validated `B` or verifies an adapter-retained byte-identical `B`; it never reconstructs bytes from metadata |

Therefore every accepted storage, recovery, and handoff path is byte-preserving by construction, while inspection remains metadata-only. M2-A2 supplies arbitrary-binary evidence, M2-A3 covers corruption and relabel attempts, M2-A4 covers storage crash boundaries, M3-A3 and M3-A6 cover descriptor transport and re-export, and M4-A3/M4-A6 cover recovered commit and release boundaries.

### Scope conclusion

The functional and technical specifications contain only generic core contracts. Milestones M0–M6 require fake or independently built contract adapters, concrete integrations remain optional external deliverables, all implementation tasks trace to this plan, and M0 may begin subject to normal milestone ordering and acceptance rules.

## Acceptance records

Each milestone evaluation appends:

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

## Plan maintenance

Implementation tasks follow milestone order. A contract change updates the authoritative specification and relevant ADR before the plan or code. Newly discovered work stays with the milestone that owns its contract. Concrete integration work is not moved into core milestones for convenience.
