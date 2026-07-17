# To Do

## Project foundation

- [x] Write the project README.
- [x] Add the MIT license.
- [x] Add instructions for AI agents.
- [x] Define the documentation structure.
- [x] Create git repo
- [x] Publish repo to GitHub and add ssh key

## Functional design

- [x] Write the functional specification.
- [x] Define profiles, tasks, desktop-entry lists, logical workspace groups, and generated session naming.
- [x] Define running, paused, stopped, and active semantics.
- [x] Specify start, switch, pause, resume, stop, and recovery behavior.
- [x] Define opaque adapter-owned checkpoint guarantees and user-visible outcome qualifiers.
- [x] Define failure cases, recovery behavior, and attention feedback.
- [x] Confirm the scope and non-goals of the first version.

## Technical design

- [x] Write the technical specification.
- [x] Choose the implementation language and project structure.
- [x] Define profile, task, adapter-registry, runtime, transaction, and checkpoint formats.
- [x] Define the versioned CLI and IPC interfaces.
- [x] Define the platform-independent core and external adapter/client dependency boundary.
- [x] Define the generic subprocess adapter protocol and conformance boundary.
- [x] Define logging, error handling, and recovery guarantees.
- [x] Define a safe testing strategy that does not disrupt the active desktop session.

## Implementation plan

- [x] Write the implementation plan.
- [x] Split the MVP into independently testable milestones.
- [x] Define acceptance criteria for each milestone.
- [x] Identify external dependencies and optional post-release downstream installer composition.

## Adapter-contract decision closure

These decisions were accepted and propagated before M3 implementation begins.

- [x] Require a terminal completed capture to retain and byte-exactly re-export its proposal and payload through a fresh core-owned descriptor; keep incomplete or unverifiable data non-authoritative.
- [x] Load `adapters.toml` at startup and reload it only explicitly and atomically with ETag/conflict checks, immutable invocation registration generations, and guards for pending intents, accepted/indeterminate operations, and release obligations.
- [x] Retain accepted and terminal adapter operation records without expiry; create a durable release obligation only for an accepted adapter operation's terminal core commit, clear it only after idempotent `operation.release`, and interpret `not_found` according to the core-recorded operation phase.
- [x] Propagate the adapter lifecycle, registry, upgrade, retention, and release decisions through the functional and technical specifications, relevant ADRs, implementation plan, and detailed M2/M3/M4 tasks.

## Implementation execution rules

- Work in milestone order and do not start a milestone until all of its documented predecessors are accepted.
- Before each task, read its linked milestone, acceptance criteria, and technical or functional reference sections in full.
- Keep each task in a separate reviewable change; do not implement a later unchecked task merely because its future interface is visible.
- Add or update the smallest test that proves the task, using only the test environment permitted by the linked testing strategy.
- If implementation requires a contract change, update the authoritative specification and affected decision record before changing the implementation plan or code.
- Check a task only after its implementation and focused tests pass. Check a milestone acceptance task only after every criterion has evidence in the implementation plan.
- Never run a live-desktop mutation as part of implementation. Live validation belongs to the explicit approval-gated release tasks.

## MVP implementation

Complete milestones in order. Each task must use the linked specification and must not introduce a concrete platform dependency.

### M0 — Safe core foundation

References: [milestone](docs/implementation-plan.md#m0--safe-core-foundation), [acceptance](docs/implementation-plan.md#m0-acceptance), and [testing](docs/technical-specification.md#testing-strategy).

- [ ] Create the Rust 2024 Cargo library and executable package.
- [ ] Create domain, application, storage-port, adapter-contract, and interface modules with inward dependency direction.
- [ ] Add architecture tests rejecting concrete platform types and dependencies in the core.
- [ ] Pin the Rust toolchain and add formatting, linting, build, documentation, and all-target test commands.
- [ ] Implement deterministic clock and UUID fakes.
- [ ] Implement canonical temporary XDG roots, ownership sentinels, containment checks, and safe cleanup.
- [ ] Add installed-adapter, display, compositor, user-bus, process-manager, home, and state trap endpoints.
- [ ] Make library tests unable to construct production adapter discovery.
- [ ] Add network-disabled CI for all M0 checks.
- [ ] Evaluate every M0 acceptance criterion and append the acceptance record.

### M1 — Domain and configuration

References: [milestone](docs/implementation-plan.md#m1--domain-and-configuration), [acceptance](docs/implementation-plan.md#m1-acceptance), [profiles](docs/functional-specification.md#profiles-and-tasks), and [configuration](docs/technical-specification.md#configuration).

- [ ] Implement name-identified profiles with ordered desktop-entry lists, tasks with ordered parent-profile subsets, launch sources, session/checkpoint UUIDs, and sessions as logical workspace groups.
- [ ] Implement running/paused lifecycle separately from the active-session role.
- [ ] Implement generated naming and independent per-source numbering.
- [ ] Implement unsigned 32-bit highest-live-plus-one allocation, lower-gap non-reuse, and `conflict.session_number_exhausted`.
- [ ] Implement stopped-number reuse and pure number/name allocation from running and paused sessions only; uncommitted transaction candidates reserve nothing.
- [ ] Enforce globally unique generated names across live sessions, rejecting delimiter-derived cross-source collisions with `conflict.session_name_collision` before mutation and without number skipping.
- [ ] Implement profile rename propagation for every related running or paused session.
- [ ] Implement task rename propagation for that task's running or paused sessions.
- [ ] Implement collision validation before rename mutation.
- [ ] Implement profile and task deletion guards for all live references.
- [ ] Treat an explicit profile/task delete request as confirmation, expose no force/cascade field, and let interactive clients prompt only before sending the request.
- [ ] Implement the finalized strict TOML schema and stable validation paths.
- [ ] Implement the 1 MiB configuration, profile/task/desktop-entry count, name, desktop-entry ID, and lexical root-path bounds.
- [ ] Implement zero-profile representation without a profiles array placeholder.
- [ ] Implement the lossless TOML syntax document and smallest-boundary edits.
- [ ] Implement exact-byte ETags and accepted file-identity checks.
- [ ] Implement offline validation independently from daemon, runtime, and adapter access.
- [ ] Implement the pure configuration-reload candidate diff/policy against a supplied runtime revision and live-reference snapshot.
- [ ] Implement atomic configuration-file adoption with expected configuration ETag; defer mutation serialization and joint runtime-lock integration to M4.
- [ ] Diff accepted and candidate configuration identity sets during reload; reject removal or apparent rename of any profile/task referenced by a running or paused session.
- [ ] Permit reload additions, non-identity edits, and removal of unreferenced sources only after complete candidate validation and deletion/collision guards.
- [ ] Add M1 reload-policy/storage tests for referenced/unreferenced removal, apparent rename, non-identity edits, concurrent file replacement, and all-or-nothing configuration adoption.
- [ ] Add domain property and lifecycle-independent state-machine tests.
- [ ] Add sequential FDS/QMK naming, rename, collision, reuse, and uncommitted-candidate fixtures, including profile `A-B` versus task `B` of profile `A`.
- [ ] Add highest-live-plus-one, lower-gap non-reuse, maximum-number, exhaustion, and stopped-number fixtures.
- [ ] Add configuration byte/count boundary and lexical root normalization fixtures.
- [ ] Add noncanonical TOML fixtures and byte-range preservation tests for every CRUD operation.
- [ ] Add missing, unreadable, symlink-replaced, inode-replaced, and byte-modified configuration tests.
- [ ] Evaluate every M1 acceptance criterion and append the acceptance record.

### M2 — Runtime and opaque checkpoint storage

References: [milestone](docs/implementation-plan.md#m2--runtime-and-opaque-checkpoint-storage), [acceptance](docs/implementation-plan.md#m2-acceptance), and [storage](docs/technical-specification.md#persistent-runtime-and-checkpoint-storage).

- [ ] Implement strict runtime JSON, monotonic revisions, shutdown marker, `max_observed_unix_ms`, attention, recovery, and pending-operation records.
- [ ] Implement transport-neutral persisted adapter IDs, format IDs, protocol versions, and the exact public session-operation kinds, monotonic phases, outcomes, conditional error/timestamps, and last-updated revision before subprocess DTOs.
- [ ] Implement durable pending lifecycle intents and separate adapter-release obligation records.
- [ ] Persist `handoff_complete` and durable `original_invocation_exit_observed` proof; permit only the spawning daemon after exact-child reaping to create the proof, and permit later daemons to trust it.
- [ ] Encode referential rules that require live targets for every pending lifecycle intent, forbid a pending start intent, and treat terminal-operation and release-obligation target UUIDs as historical correlation after session/checkpoint removal.
- [ ] Implement 30-day/1,000-record terminal public operation history retention without removing recovery evidence, using the durable maximum of previously observed and current wall-clock time.
- [ ] Add deterministic age, count, revision-order, backward-clock, forward-clock, restart, `operation.get` expiry, and release-independence history tests proving clock rollback cannot extend retention.
- [ ] Add operation state-machine fixtures for every legal phase transition and reject backward phases, premature outcomes/timestamps, invalid completed-with-limitations kinds, and missing required live targets.
- [ ] Add stopped-session and successful-discard fixtures proving terminal operation history and release obligations remain valid without a live target.
- [ ] Implement runtime count and 8 MiB byte bounds, attention coalescing, worst-case pre-effect capacity reservation, prospective rejection, and the read-only gate for externally over-bound state.
- [ ] Implement transport-neutral configuration/runtime projection-size components using recursively UTF-8-byte-sorted object keys and the shared canonical compact JSON rules, without depending on M3 capability or registry types; M4 will compose the exact combined messages.
- [ ] Persist the core operation UUID and complete immutable adapter intent before any adapter invocation.
- [ ] Make pending-operation recovery independent of whether an optional adapter operation token was received.
- [ ] Implement previous validated runtime retention and revision-based selection.
- [ ] Implement the exact version 1 checkpoint envelope, 16 KiB envelope bound, 64 MiB payload bound, SHA-256 integrity, and strict enums.
- [ ] Validate checkpoint/session IDs as canonical UUIDs and adapter/format IDs with the machine-identifier grammar.
- [ ] Implement UUID-derived checkpoint paths and atomic opaque payload storage.
- [ ] Implement payload length and digest verification without semantic parsing.
- [ ] Implement configuration/runtime/checkpoint transaction manifests and complete-set recovery.
- [ ] Implement reference-safe checkpoint retention, unsupported-format rejection without migration, quarantine, and deletion.
- [ ] Add arbitrary empty, binary, non-UTF-8, and boundary-sized payload fixtures.
- [ ] Prove byte-exact round-trip across storage, restart, backup selection, and handoff.
- [ ] Add digest, length, relabel, path traversal, unsupported version, and orphan-reference tests.
- [ ] Add deterministic failure points around every candidate, sync, manifest, replace, reference, and cleanup boundary.
- [ ] Add log and inspection tests proving payload bytes and secrets are never exposed.
- [ ] Evaluate every M2 acceptance criterion and append the acceptance record.

### M3 — Generic adapter contract and conformance kit

References: [milestone](docs/implementation-plan.md#m3--generic-adapter-contract-and-conformance-kit), [acceptance](docs/implementation-plan.md#m3-acceptance), and [adapter contract](docs/technical-specification.md#generic-adapter-contract).

- [ ] Reuse the M2 transport-neutral adapter ID, format ID, protocol-version, and persisted operation types without redefining them in M3.
- [ ] Implement M3-owned capability IDs and capability-qualified format ranges so support advertised for one capability implies nothing about another.
- [ ] Define empty capability format lists as format-independent; enforce unique capability IDs, at most one entry per format ID within a capability, and `min_version <= max_version` for every inclusive non-empty range.
- [ ] Implement every normative adapter protocol string, collection, path, code, limitation, and message bound.
- [ ] Implement exact typed subprocess requests for `hello`, `compatibility.inspect`, `operation.status`, `operation.release`, `checkpoint.capture`, `checkpoint.restore`, and `checkpoint.discard` over the M2 transport-neutral types.
- [ ] Implement exact typed accepted, final result, checkpoint proposal, limitation, compatibility, status, and protocol-error objects.
- [ ] Reject duplicate JSON keys, unknown fields, invalid UTF-8, trailing data, invalid UUIDs, invalid integers, and unsupported protocol values.
- [ ] Implement strict mode-`0600` startup decoding of `adapters.toml` with exact accepted bytes, owner, file identity, and SHA-256 source ETag.
- [ ] Model a canonical adapter-generation UUID separately from the source ETag and assign a fresh generation to the accepted source/projection pair at every daemon startup.
- [ ] Implement the adapter-registry projection-size component with the shared canonical compact JSON rules, including worst-case capabilities, formats, diagnostics, recursively UTF-8-byte-sorted object keys, escaping, and terminating LF, without composing application snapshots yet.
- [ ] Implement the 256 KiB registry, 128-adapter, 64-environment-name, path, environment-name/value bounds, invalid-value pre-spawn failure, and missing-file ETag.
- [ ] Implement `adapter.reload` with expected source ETag and adapter generation plus two on-disk identity/byte comparisons.
- [ ] Validate the complete reload candidate and atomically publish all or none of the accepted source/projection pair.
- [ ] On reload, create a new adapter generation and `adapter_registry.changed` event when the accepted source ETag or public projection changes; preserve the generation and publish no event only for an exact source-and-projection no-op.
- [ ] Add reload tests for source-only changes, projection changes with unchanged source ETag, combined changes, and exact no-ops.
- [ ] Treat a missing reload candidate as an empty registry and reject unreadable, invalid, or concurrently changed candidates without replacing the accepted generation.
- [ ] Reject removal or identity-sensitive changes for adapters referenced by any pending lifecycle intent, accepted or indeterminate operation, or adapter-release obligation.
- [ ] Permit independent adapter additions during pending operations.
- [ ] Pin every spawned invocation to one immutable accepted registration generation while explicitly not claiming that the generation pins executable bytes across separate invocations.
- [ ] Open the registered executable without following symlinks for every requested invocation; define file identity from device, inode, nanosecond change time, size, mode, owner, and group; recheck it before execution; cache hello only by that identity plus accepted adapter generation; and for an uncached identity/generation pair run hello and the requested method from duplicate descriptors of the same image using Linux `execveat(AT_EMPTY_PATH)` or an equivalently race-free facility.
- [ ] Require adapter ID, protocol version, display name, capabilities, and format ranges to be deterministic for one executable identity and accepted generation; run the requested method for an uncached identity/generation pair only when hello exactly matches the accepted public projection, otherwise report the adapter unavailable until explicit reload.
- [ ] Keep public adapter availability equal to the latest startup/reload validation result; report transient invocation failures through that request and attention without silently changing the public projection or generation.
- [ ] Support adapter upgrades only by atomic registered-path replacement; detect and fail pre-handoff when the identity of an already opened inode changes during spawn rather than claiming safe concurrent in-place byte mutation.
- [ ] Add spawn race tests proving that path replacement after open cannot substitute an inode, cached hello never authorizes a different identity or generation, and a new image cannot receive the requested method unless hello from a duplicate of that image exactly matches every accepted public metadata field.
- [ ] Validate canonical absolute regular executable paths, trusted ownership, permission modes, duplicate IDs, and reported identity.
- [ ] Reject PATH lookup, directory discovery, symlink executables, shell arguments, and partial registry acceptance.
- [ ] Implement the minimal built-in environment allowlist.
- [ ] Implement validated explicit environment forwarding and forbidden secret/internal name rejection.
- [ ] Implement one-request-per-invocation subprocess spawning with the exact executable arguments and no shell.
- [ ] Implement 1 MiB bounded stdin/stdout JSON Lines framing and request-ID correlation.
- [ ] Implement bounded sanitized stderr collection without logging environment values or payload bytes.
- [ ] Implement read-only `hello` discovery and exact adapter-ID verification.
- [ ] Implement metadata-only compatibility inspection with no payload descriptor.
- [ ] Require the application port invocation to carry an already persisted core operation UUID and reject transport invocation without that precondition; leave durable intent creation to M2/M4.
- [ ] Implement optional opaque operation tokens as secondary correlation data only.
- [ ] Accept exactly two mutation sequences: one permitted pre-acceptance final (`unsupported`, `rejected`, `failed`, or `cancelled`) with no effect, or `accepted` followed by one final; reject pre-acceptance success, duplicate phases, and effect-before-acceptance evidence.
- [ ] For capture, let the adapter choose one deterministic proposal per operation UUID and reject completed/re-exported proposals outside `checkpoint.capture` ranges or inconsistent with prior results.
- [ ] Require a capture/restore format intersection before pause and validate the chosen proposal against both; reject creation of a checkpoint that is not initially restorable.
- [ ] When `discard_requires_adapter = true`, validate the chosen format against `checkpoint.discard`; require every live-preservation proposal to set adapter-required discard.
- [ ] Require `activity = suspended` to have `session.suspend` and to match its non-empty format ranges; treat empty suspend ranges as format-independent.
- [ ] In the standalone sample adapter, implement accepted and terminal status persistence and lookup by core operation UUID across process exit, restart, and compatible upgrade.
- [ ] In the standalone sample adapter, retain operation status and method-specific recovery data without age-based expiry.
- [ ] Implement one-final-response idempotent `operation.release` in the adapter protocol without an `accepted` phase.
- [ ] In the standalone sample adapter, delete retained recovery data on release without creating a permanent tombstone.
- [ ] In the standalone sample adapter, make release of an existing or already absent operation return `released: true`.
- [ ] Add conformance cases proving status is queried only for pending lifecycle intents: post-handoff `not_found.operation` is non-authoritative while the original child may still run or after restart without proof, becomes known non-acceptance only after durable exact-child exit proof, and is a protocol violation for a pending intent after recorded acceptance; terminal release obligations invoke only idempotent release, where absence returns `released: true`.
- [ ] In the standalone sample adapter, require terminal state persistence before writing its final stdout envelope.
- [ ] Implement capture output descriptor mapping without fixed descriptor assumptions or checkpoint paths.
- [ ] Implement restore/discard read-only input descriptor mapping at byte offset zero.
- [ ] Implement 64 MiB bounded payload transfer and exact byte preservation.
- [ ] In the standalone sample adapter, persist the exact completed capture proposal and payload before writing the final stdout result.
- [ ] Add optional fresh `output_fd` handling to `operation.status` for completed capture reconciliation.
- [ ] In the standalone sample adapter, re-export the exact retained capture proposal and payload idempotently for the same core operation UUID.
- [ ] Add conformance assertions that re-exported proposal, length, SHA-256, and payload bytes exactly match the original completed capture.
- [ ] Require adapters unable to retain/re-export a capture to report the original capture as `failed`, never `completed`.
- [ ] Implement 30-second deadlines for lifecycle mutations and capture re-export status, and 5-second deadlines for metadata-only status and other read-only control methods.
- [ ] Implement the exact complete-request handoff boundary; permit termination/reaping only before a complete valid mutating line is delivered.
- [ ] After complete handoff, send neither `SIGTERM` nor `SIGKILL`; close owned descriptors as applicable, track/reap the exact child, durably commit its exit proof before relying on absent status, and persist `acceptance_unknown` or `indeterminate` without replay.
- [ ] Enforce elapsed subprocess deadlines with a monotonic clock and treat `deadline_unix_ms` as adapter context rather than core timeout authority.
- [ ] Implement post-acceptance transport closure as indeterminate without `SIGKILL` or inferred failure.
- [ ] Implement safe concurrent operation handling and same-operation parameter-reuse rejection.
- [ ] Implement the in-process application fake port for application/lifecycle tests without subprocess or wire behavior.
- [ ] Implement the fake subprocess adapter for framing, descriptor, timeout, signal, crash, malformed-output, and transport recovery tests.
- [ ] Add fake subprocess modes for partial handoff, delayed acceptance, acceptance/stdout races, unexpected-signal detection, pre/post-acceptance crash, disconnect, malformed output, delayed status, byte-exact capture re-export, premature status loss, and release.
- [ ] Implement explicit test composition with no installed-adapter discovery.
- [ ] Create an independently built sample-adapter fixture outside the production Cargo package using only the published protocol contract; exclude it from core release artifacts.
- [ ] Ensure the sample adapter imports no Session Manager internal crate or private type.
- [ ] Publish adapter-conformance identity, compatibility, capability, and version fixtures.
- [ ] Publish framing, descriptor, binary payload, bound, and confidentiality fixtures.
- [ ] Publish concurrency, timeout, cancellation, crash, reconnect, non-expiring status, explicit release, premature `not_found`, and capture re-export fixtures.
- [ ] Add negative tests for fuzzy, duplicate, relabeled, downgraded, missing, incompatible, and operation-ID-reuse adapters.
- [ ] Add tests distinguishing protocol failure, adapter failure, cancellation, timeout, disconnect, and indeterminate outcomes.
- [ ] Add conformance tests that replace the adapter executable by atomic registered-path replacement between invocations and prove a compatible upgrade preserves adapter ID, protocol behavior, unreleased status, capture re-export, and release.
- [ ] Run the conformance kit against the separately built sample adapter.
- [ ] Evaluate every M3 acceptance criterion and append the acceptance record.

### M4 — Lifecycle and recovery service

References: [milestone](docs/implementation-plan.md#m4--lifecycle-and-recovery-service), [acceptance](docs/implementation-plan.md#m4-acceptance), [lifecycle](docs/functional-specification.md#lifecycle-operations), and [recovery](docs/technical-specification.md#errors-attention-and-recovery).

- [ ] Implement overlapping-mutation serialization, concurrent reads, and stale preconditions.
- [ ] Integrate configuration reload with expected runtime revision, current live-reference guards, and the joint mutation/commit lock.
- [ ] Add reload race tests for start-versus-reload, stop-versus-reload, rename-versus-reload, repeated-reload ordering, and external configuration replacement.
- [ ] Compose the M2 configuration/runtime and M3 adapter-registry components into one canonical compact combined snapshot/event encoder without socket I/O: recursively UTF-8-byte-sorted object keys, direct non-ASCII UTF-8, exact mandatory escapes, minimal integers, no insignificant whitespace, and one size-counted terminating LF.
- [ ] Invoke the shared exact combined encoder before every application commit and reject any resulting snapshot or maximum commit event above 16 MiB with `recovery.capacity_exceeded`.
- [ ] During recovery, place an externally assembled but individually valid over-16-MiB combined snapshot under the read-only recovery gate before publication.
- [ ] Implement core-only start by calculating its number/name under the mutation lock and atomically committing UUID, session, immutable profile/task desktop-entry and root snapshot, active role, and direct-final public operation; persist no intermediate reservation and invoke no adapter or platform effect.
- [ ] Implement the single lifecycle-admission boundary after strict decoding, authorization, preconditions, reference/lifecycle/naming checks, operation-specific adapter capability/compatibility preflight, and capacity reservation; start, switch, running stop, and paused stop without adapter discard require no adapter, while every rejection before admission creates no operation UUID, record, or lifecycle event.
- [ ] Atomically create each admitted adapter-backed operation identity with its durable pending intent, and each admitted core-only operation identity with its direct-final state change, before exposing either to readers.
- [ ] Implement logical active-session switch with no platform action assumption.
- [ ] Make every successful start and completed or completed-with-limitations resume select the resulting running session as active, with no conditional “normally active” branch.
- [ ] Implement pause intent, adapter capture, payload validation, atomic checkpoint storage, and paused commit.
- [ ] Implement resume compatibility, exact payload handoff, verified outcome, and running commit.
- [ ] Implement stop cleanup/discard, checkpoint retention on uncertainty, deletion, and number release.
- [ ] Map every accepted terminal negative capture/restore/discard outcome to unchanged lifecycle, a terminal public operation, and an adapter-release obligation.
- [ ] Map accepted `completed` capture with an invalid proposal/capability relation to unchanged running lifecycle, final failed public operation with `protocol.invalid_message`, durable attention, and a release obligation.
- [ ] Implement typed durable intents before every non-atomic adapter effect.
- [ ] Atomically replace the lifecycle intent with an adapter-release obligation only in a terminal core commit for an adapter operation known to have reached `accepted`, including accepted terminal failures that leave lifecycle unchanged.
- [ ] Prove core-only operations and permitted pre-acceptance finals create no adapter-release obligation.
- [ ] Reconcile adapter status by the persisted core operation UUID even when no optional adapter token was committed.
- [ ] Reconcile a completed indeterminate capture through a fresh descriptor and commit only the byte-exact verified proposal/payload pair.
- [ ] Keep missing, partial, changed, oversized, or unverifiable capture re-export data non-authoritative and recovery-required.
- [ ] Implement adapter status reconciliation without blind non-idempotent replay.
- [ ] Process adapter-release obligations only after the final lifecycle/runtime commit is durable.
- [ ] Once a terminal accepted result replaces its lifecycle intent with a release obligation, forbid further `operation.status` calls for that operation and route recovery exclusively through idempotent `operation.release`.
- [ ] Remove a release obligation only in a later durable commit after `operation.release` completes.
- [ ] Recover and retry release obligations after crashes at every post-commit/release boundary.
- [ ] Retry an indeterminate idempotent release safely until its final result is known.
- [ ] Implement durable attention and acknowledgement plus automatic status/release reconciliation at immediate, 1, 2, 4, 8 through 300-second capped intervals and one in-flight control per operation; reset to one second only after a durable phase advance, exact-child exit proof, authoritative terminal result, or completed release, and do not reset for repeated state, unchanged absence, transport error, or acknowledgement.
- [ ] Add injected-monotonic-clock tests proving each qualifying progress transition resets reconciliation while repeated accepted/non-terminal status and repeated non-authoritative `not_found.operation` continue the capped backoff.
- [ ] Add public application-operation tests for every valid and invalid transition.
- [ ] Add admission-boundary tests proving validation, stale-precondition, missing-reference, lifecycle, generated-name collision/exhaustion, operation-specific unavailable/incompatible-adapter, and capacity failures create no public operation or lifecycle event, while core-only operations succeed with an empty/unavailable adapter registry and a crash cannot expose an admitted operation without its matching durable authority.
- [ ] Add repeated sequential same-source start and independent-session tests, including highest-live-plus-one allocation, lower-gap non-reuse while a higher live number remains, stopped-number reuse when no higher live number remains, and all-or-nothing collision failure; do not add concurrent-start behavior to the version 1 contract.
- [ ] Add configuration/runtime rename integration tests proving adapter identity, checkpoint envelope, and payload bytes remain unchanged for paused sessions.
- [ ] Add the exact capture/suspend/restore/discard capability matrix and lifecycle-result tests.
- [ ] Add empty/non-empty range intersection tests proving pause cannot commit an initially unrestorable or undiscardable-required checkpoint.
- [ ] Add crash/restart tests at every adapter and persistence boundary.
- [ ] Add missing, incompatible, failed, cancelled, timed-out, and indeterminate adapter scenarios.
- [ ] Add the exact pre-handoff failure, post-handoff acceptance-unknown, accepted-indeterminate, durable-exit-proof/absent, and restart-without-exit-proof lifecycle matrix.
- [ ] Add status/release recovery tests proving: a live original child remains recovery-required; restart without durable exit proof remains recovery-required; committed exact-child exit proof permits known non-acceptance; status absence for a pending accepted intent creates durable protocol-violation attention; terminal release obligations never query status; and lost release responses converge through absent-record `released: true`.
- [ ] Evaluate every M4 acceptance criterion and append the acceptance record.

### M5 — Local daemon, IPC, and CLI

References: [milestone](docs/implementation-plan.md#m5--local-daemon-ipc-and-cli), [acceptance](docs/implementation-plan.md#m5-acceptance), and [interfaces](docs/technical-specification.md#cli-and-ipc).

- [ ] Implement foreground daemon startup, single-instance lock, recovery gate, and orderly shutdown.
- [ ] Implement owner-only Unix socket creation and same-user peer checks.
- [ ] Implement the exact version 1 request, response, error, subscribe acknowledgement, snapshot, event, and subscription-error envelope types; make `event.subscribe` parameterless, reject buffered pipelined trailing bytes before acknowledgement, and after its response make that connection server-stream-only and close it without response on any subsequent client byte.
- [ ] Implement the canonical compact UTF-8 JSON Lines encoder and 16 MiB framing bound including LF, with recursively UTF-8-byte-sorted object keys, exact mandatory escaping, direct non-ASCII UTF-8, minimal integers, duplicate-key rejection, trailing-data rejection, and size rejection; accept noncanonical member order and spacing in otherwise valid incoming requests.
- [ ] Enforce the public client-name, stable-code, field-path, message, cursor, domain-value, collection, and projection bounds.
- [ ] Require `system.hello` as the first connection method.
- [ ] Implement exact version negotiation and unsupported-version connection closure.
- [ ] Implement request UUID validation and duplicate/reused request-ID rejection.
- [ ] Implement the stable error object and map every version 1 error family.
- [ ] Map safely final pre-handoff/pre-acceptance timeout to `timeout.pre_acceptance` and unresolved post-handoff ambiguity to `outcome.acceptance_unknown`, both with the documented exit codes.
- [ ] Implement configuration ETag and runtime revision precondition validation before mutation.
- [ ] Require both configuration ETag and runtime revision for configuration reload, profile/task rename, and profile/task delete; keep configuration-only edits on the configuration ETag.
- [ ] Implement list limits, stable ordering, opaque cursors, and stale-cursor rejection.
- [ ] Implement `system.hello`, `system.status`, `config.get`, `config.validate`, and `config.reload` schemas and handlers.
- [ ] Make successful `config.reload` return the unchanged current runtime revision and prove it never publishes or persists a runtime mutation.
- [ ] Implement profile list/get/create/update/delete schemas and handlers.
- [ ] Implement task list/get/create/update/delete schemas and handlers.
- [ ] Implement session list/get/start/switch/pause/resume/stop schemas and handlers.
- [ ] Enforce exact start, pause, resume, and stop parameter authority at schema validation.
- [ ] Implement adapter list/get/compatibility schemas and metadata-only results.
- [ ] Implement `adapter.reload` schema, source-ETag and adapter-generation preconditions, atomic handler, and CLI command.
- [ ] Include the adapter-registry source ETag and adapter generation in hello/status, adapter list/get, errors, snapshots, and event revision tuples.
- [ ] Publish `adapter_registry.changed` only with a changed adapter generation, and add stale adapter-cursor tests including a generation change whose source ETag is unchanged.
- [ ] Implement operation get and attention list/acknowledge schemas and handlers.
- [ ] Implement the exact public operation projection, admitted core-only direct-final behavior, conditional fields, and historical target UUID behavior for `operation.get` and session mutation results; synchronous pre-admission errors contain no operation ID.
- [ ] Implement unconditional snapshot-first subscription acknowledgement with no last-seen tuple or resume branch.
- [ ] Under the publication lock, atomically register the subscriber queue, capture one coherent runtime-revision/configuration-ETag/adapter-registry source-ETag/adapter-generation tuple, and enqueue its complete initial snapshot before allowing later events.
- [ ] Add deterministic subscription-registration race tests proving every concurrent commit appears either in the initial snapshot or in a later queued event and can never be lost between snapshot capture and queue registration.
- [ ] Implement the closed event-kind vocabulary as ordered unique `kinds`; require `runtime.changed` for every runtime replacement and allow `operation.changed` or `attention.changed` only as additional cause qualifiers beside it.
- [ ] Implement exact event `projections` fields with null for unchanged configuration/registry/runtime projections and kind-conditioned non-null complete replacements.
- [ ] Publish profile/task rename as one atomic configuration/runtime event and test that no subscriber observes mixed labels.
- [ ] Implement per-connection revision ordering and subscription queues bounded to both 64 messages and 33,554,432 encoded bytes from initial-snapshot enqueue onward, including that queued snapshot and excluding only the message currently being written.
- [ ] Enforce both a five-second monotonic no-forward-progress deadline and a 30-second total deadline for the subscription acknowledgement and every snapshot/event/error write; restart only the stall deadline on actual byte progress and close immediately when either expires.
- [ ] On either queue overflow, atomically discard queued events; if no frame is partial make one nonblocking best-effort attempt from reserved capacity to write the exact compact `subscription_error`, otherwise append nothing; then close and require a fresh snapshot connection.
- [ ] Implement `server.shutting_down` publication.
- [ ] Implement matching CLI command parsing for every IPC method.
- [ ] Implement stable `--json` result/error output without human wrappers.
- [ ] Implement human output separately from the machine contract.
- [ ] Implement exact success, validation, not-found, conflict, adapter, persistence, timeout, indeterminate, unavailable-daemon, and internal exit codes.
- [ ] Implement client timeout behavior without cancelling or signalling handed-off acceptance-unknown or accepted operations.
- [ ] Expose safe checkpoint metadata without payload or platform-state inspection.
- [ ] Publish complete revisioned configuration, session-as-workspace-group, adapter capability/format, operation, and attention projections with no client-specific derived model.
- [ ] Enforce core-only start/switch parameters, explicit pause adapter selection, and persisted-adapter resume/paused-stop routing in IPC schemas.
- [ ] Add malformed, fragmented, coalesced, concurrent, oversized, slow-client, and incompatible-protocol tests.
- [ ] Add handshake-order, duplicate request-ID, subscription-parameter rejection, pipelined-trailing-byte rejection, post-subscribe client-byte closure, exact subscribe-acknowledgement/snapshot, unknown-event, pagination-limit, stale-cursor, maximum-valid-snapshot, and over-limit source-document tests.
- [ ] Add canonical-encoding golden tests for recursive object-key order, every escape class, non-ASCII UTF-8, integer spelling, compactness, terminating LF, and acceptance of noncanonical incoming request order/spacing; add worst-case combined snapshot/event tests and reject any prospective commit exceeding the 16 MiB public line bound.
- [ ] Add slow-client tests crossing the message-count and byte-count queue limits independently, stalling initial and partial writes, trickling progress, advancing the injected monotonic clock, and proving resynchronization is attempted only for a clean frame while the five-second stall and 30-second total deadlines bound every path.
- [ ] Add real temporary-socket CRUD, lifecycle, adapter-failure, restart, and reconnect tests using only the fake subprocess adapter.
- [ ] Add public-schema tests rejecting concrete platform, window, process, layout, shell, and shortcut fields.
- [ ] Add downstream-client fixtures using only generic snapshots and lifecycle methods.
- [ ] Evaluate every M5 acceptance criterion and append the acceptance record.

### M6 — Core hardening and standalone release

References: [milestone](docs/implementation-plan.md#m6--core-hardening-and-release), [acceptance](docs/implementation-plan.md#m6-acceptance), and [technical acceptance](docs/technical-specification.md#technical-acceptance).

- [ ] Build a requirements trace from every retained contract to passing test evidence.
- [ ] Close uncovered domain, parser, persistence, adapter, lifecycle, IPC, recovery, and privacy cases.
- [ ] Audit dependencies and public schemas for concrete platform coupling.
- [ ] Run clean network-disabled default checks with every live and installed-adapter trap enabled.
- [ ] Run the complete fake-adapter and independently built sample-adapter conformance suites.
- [ ] Audit logs, fixtures, state, and artifacts for payload bytes, credentials, and private paths.
- [ ] Package the core daemon, CLI, example configuration, service definition, adapter contract, and conformance kit without a concrete adapter.
- [ ] Document core installation, configuration, lifecycle, adapter development, compatibility, recovery, upgrade, rollback, and uninstall.
- [ ] Verify installation, upgrade, rollback, and uninstall preserve existing configuration and checkpoints by default.
- [ ] Build reproducible artifacts and record checksums.
- [ ] Verify artifacts in a fresh non-graphical environment.
- [ ] Evaluate every M6 acceptance criterion and append the acceptance record.

## Release

- [ ] Freeze the release commit after M6 acceptance.
- [ ] Rerun every release-gating check from the exact release revision.
- [ ] Verify license, notices, package contents, example configuration, and absence of generated or private files.
- [ ] Write release notes covering generic capabilities, adapter requirements, limitations, upgrade, and rollback.
- [ ] Create and push the first signed version tag.
- [ ] Publish reproducible core artifacts, checksums, documentation, and the adapter-conformance kit.

## Optional external integrations

These tasks are not part of the core MVP or first-release acceptance.

- [ ] Design and implement any concrete platform adapter in its owning project against the released generic contract.
- [ ] Keep platform checkpoint schemas and compatibility logic inside their owning adapter projects.
- [ ] Implement presentation and input behavior only as downstream generic API clients.
- [ ] Implement optional process preservation only as adapter-owned behavior expressed through generic capabilities.
- [ ] Add downstream installer composition only after tagged core and integration releases are available.
