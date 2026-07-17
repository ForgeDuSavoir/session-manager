# Functional Specification

## Purpose

This document is the authoritative source for the user-visible, platform-independent behavior of Session Manager.

Session Manager organizes reusable work contexts. It records profiles, optional tasks, desktop-entry lists, logical workspace groups, named sessions, and resumable checkpoints. It does not implement a compositor, desktop shell, launcher, layout, keyboard shortcuts, window manager, or process manager.

Concrete integrations may create workspaces, launch applications, preserve windows, suspend processes, or present the user interface. They do so as clients or adapters of Session Manager's generic contracts and remain responsible for their platform-specific behavior.

## Status

This specification follows [ADR 0010](decisions/0010-keep-the-core-platform-independent-with-opaque-adapter-checkpoints.md). The platform-independent product contract is complete; concrete integrations may add behavior only outside the core contracts defined here.

## Product terminology

- **Session profile:** a named, durable, user-managed work-context definition containing a root directory, a possibly empty desktop-entry list, and a possibly empty list of profile tasks. It contains no live desktop state.
- **Profile task:** a named preset within a profile containing an ordered, unique list of desktop-entry IDs selected from its parent profile. It contains no window or workspace placement.
- **Desktop-entry ID:** a reference to an application launch definition. Session Manager stores and exposes the ID; a client or adapter decides whether and how to resolve or launch it.
- **Logical workspace group:** the stable UUID-backed group represented by one session. The group has no core-visible members, slots, count, roles, or global/special variants. A client or adapter maps the session UUID to any concrete workspaces it owns.
- **Launch source:** either a profile or one task within a profile.
- **Session:** one numbered runtime instance created from a launch source.
- **Active session:** the single session selected as the current logical context. Activity is a core role, not proof that a desktop integration is currently displaying it.
- **Running session:** a live logical session that has not committed a checkpoint as paused and has not stopped.
- **Paused session:** a resumable session whose checkpoint has been durably accepted by Session Manager.
- **Stopped session:** a completed session with no retained checkpoint or runtime record.
- **Adapter:** an independently implemented component that performs optional platform-specific lifecycle work through the generic adapter contract.
- **Checkpoint envelope:** core-owned metadata identifying the producing adapter and payload format and protecting the stored payload.
- **Checkpoint payload:** opaque bytes owned exclusively by the producing adapter.
- **Client:** a shell, launcher, compositor configuration, command-line tool, automation process, or other consumer of the public API.
- **Recovery:** reconciliation of core state, pending operations, checkpoint integrity, and adapter-reported status after interruption.

Running, paused, active, and stopped are not interchangeable. Switching the active session does not pause or stop the previous session.

## Profiles and tasks

Users can create, inspect, modify, rename, and delete profiles.

A profile requires:

- a unique non-empty name;
- one root directory value;
- a desktop-entry list, which may be empty;
- a task list, which may be empty.

Each task requires a name unique within its profile and an ordered, unique desktop-entry list whose entries must also exist in the parent profile. The list may be empty. A task cannot contain workspace destinations, window declarations, compositor identifiers, coordinates, layout state, shell widgets, keyboard shortcuts, process IDs, or platform commands.

The root directory is contextual data made available to clients and adapters. Session Manager does not assume that every application supports it and does not itself change another process's working directory.

The profile desktop-entry list expresses applications available in its context. A task list expresses the applications suggested by that preset. Session Manager does not resolve or execute them; clients decide whether and how to launch them after observing a new session.

Every session is itself one logical workspace group. Fixed counts, numbered slots, special workspaces, global workspaces, task window destinations, and concrete membership are downstream desktop policy and never profile or task fields.

Profile and task edits affect future starts. They do not rewrite an accepted checkpoint. Renaming is the sole live presentation exception: a supported profile or task rename updates the source label and generated name of every related running or paused session while preserving its UUID, number, source association, lifecycle, and checkpoint bytes.

A task cannot be deleted while referenced by a running or paused session. A profile cannot be deleted while it or one of its tasks is so referenced. Submitting the explicit profile/task delete API request is the confirmation; interactive clients may ask the user before submitting it, but the protocol has no separate confirmation or force flag. Deletion never instructs an adapter to destroy external state implicitly.

## Session identity and naming

A profile and each of its tasks are distinct launch sources. Multiple running or paused sessions may share one source.

Names are generated automatically:

- profile session: `<profile-name>-<number>`;
- task session: `<profile-name>-<task-name>-<number>`.

The number is calculated independently per launch source in the range 1 through 4,294,967,295. Running and paused sessions reserve their numbers; stopped instances do not. A new session uses `1` when no number is reserved; otherwise it uses one more than the highest live number. Exhaustion fails without creating a session; lower gaps are not reused while a higher live number remains. Generated names are globally unique among live sessions. If distinct profile/task name combinations produce the same generated text, start fails without creating a session instead of changing the allocation rule or silently allowing duplicate names. A start never exposes a partially created session or a separately visible reservation.

Examples include `FDS-1`, `FDS-MontageVidéo-1`, `FDS-MontageVidéo-2`, `FDS-MiniaMaking-1`, `QMK-1`, and `QMK-2`.

If a session stops, its number no longer participates. If `QMK-1` is the only live `QMK` session and stops, the next `QMK` start is again `QMK-1`. If `QMK-2` remains running or paused, the next is `QMK-3`.

Renaming a profile updates the profile segment of all related running and paused sessions. Renaming a task updates only sessions from that task. The operation is rejected before mutation if any resulting profile key, task key, or generated live name would collide.

## Lifecycle model

The persisted lifecycle is:

- **Running:** the core session exists and has not been paused or stopped.
- **Paused:** the core has durably committed one checkpoint envelope and payload accepted from the selected adapter.
- **Stopped:** the runtime instance and checkpoint are removed after successful completion and the instance no longer participates in naming.

At most one running session is active. Paused sessions are inactive. No active session is a valid state.

Adapter capability and outcome qualify what an operation achieved externally. They do not create hidden lifecycle states. A partial, unsupported, failed, or indeterminate adapter result must be visible and must not be translated into a stronger lifecycle claim.

## Lifecycle operations

A lifecycle request is admitted only after its syntax, authorization, preconditions, referenced source or session, lifecycle guards, required adapter compatibility, generated-name availability, and capacity have been validated. A rejection at that boundary is a synchronous error and creates no public operation. Once admitted, the operation identity and the authority needed to recover it are committed atomically before any adapter effect; core-only operations commit their final public operation together with their lifecycle change.

### Start

Starting a profile or task reserves a number, snapshots the source identity and relevant platform-independent configuration, creates a running session, and makes it active.

Start is a core-only operation. Session Manager does not invoke an adapter, create compositor workspaces, launch `.desktop` files, place windows, focus a surface, or create process groups. After the committed session event, clients may map the session UUID to workspaces and may use the source snapshot's desktop-entry IDs and root directory according to their own policy.

Starting never resumes a stopped instance or reuses a discarded checkpoint. Starting the same source again creates a distinct session with a separate UUID and live number.

### Switch

Switch selects an existing running session as the active logical context. The previously active session remains running. Session Manager publishes the new active identity; clients decide how that affects their own UI, compositor state, launcher, or shortcuts.

Switch never invokes an adapter or platform-specific navigation API. A failed downstream presentation change does not silently rewrite core lifecycle state.

### Pause

Pause requires a selected compatible adapter with `checkpoint.capture`. The core commits `paused` only after it has durably stored a valid envelope and the exact payload bytes from a completed capture.

Version 1 does not let the caller choose a checkpoint format. Before capture, the selected adapter must advertise at least one format/version usable for both capture and restore. The adapter then chooses such a format, and the core rejects a completed proposal outside both advertised ranges. If adapter discard is declared necessary, that same format must also be advertised for discard; live preservation always requires adapter discard. The choice is stable for the operation UUID and must be reproduced exactly during recovery. A format-independent capability is represented by an empty format list.

The capture result declares `preservation = snapshot` or `preservation = live`, and `activity = unchanged` or `activity = suspended`. An adapter may report `suspended` only when it also declares `session.suspend` and verifies suspension. These qualifiers are retained and shown to clients. Core `paused` means only that a resumable checkpoint is stored; it never implies suspended processes or exact restoration.

If capture is unsupported, rejected, fails, is cancelled, or returns an invalid format/capability proposal, the session remains running. An invalid accepted completion is exposed as a protocol-invalid attention condition rather than being promoted to paused. A terminal result after acceptance still requires adapter-record release. A timeout or disconnect after complete request handoff but before observed acceptance is acceptance-unknown; after known acceptance it is indeterminate. In both cases the session remains running with a pending recovery condition until adapter status resolves. No checkpoint candidate becomes authoritative merely because bytes were partially received.

### Resume

Resume applies to a paused session. Session Manager verifies the checkpoint envelope and payload integrity and offers the unchanged payload only to a directly compatible adapter. The adapter owns interpretation, restoration, and verification. Version 1 performs no checkpoint migration; future migration remains adapter-owned and requires a new core contract.

`checkpoint.restore` is required. A completed restore changes the session to running and makes it active. A completed restore with limitations also changes it to running and active but retains an attention condition and the adapter's restoration qualifier. Unsupported, rejected, failed, or cancelled restore, whether before or after acceptance, leaves the session paused; a terminal result after acceptance still requires adapter-record release. An acceptance-unknown or indeterminate restore leaves it paused and recovery-required until status resolves. The core never falls back to the current profile definition, chooses another adapter, or claims exact restoration without adapter verification.

Profile or task edits made after capture do not modify the retained checkpoint and do not affect that resume.

### Stop

Stopping a running session is a core-only operation: it removes the logical session and does not close windows, remove workspaces, or terminate applications. Downstream clients are responsible for any external cleanup they choose.

Stopping a paused session examines its envelope. When `discard_requires_adapter = false`, the core atomically removes the checkpoint and session without invoking the adapter. When it is true, the producing compatible adapter must support `checkpoint.discard` and report completion before core deletion. Session Manager never invents cleanup steps or interprets the payload.

Unsupported, rejected, failed, acceptance-unknown, or indeterminate required discard retains the paused session and checkpoint with an attention condition. A later explicit stop may begin a new discard only when no unresolved prior discard exists and after a fresh compatibility check; automatic recovery queries status for delivered unresolved operations and never repeats discard.

After a completed stop, a later start uses the profile or task definition and the current numbering set; it does not restore the stopped checkpoint.

### Recovery

Recovery validates core configuration, runtime state, checkpoint envelope metadata, payload length and integrity, pending operations, and available adapter compatibility. It queries adapter operation status by the core operation UUID that was persisted before invocation. An optional opaque adapter token may improve diagnostics or correlation but is never required for recovery.

When a completed capture must be recovered after transport loss, the adapter re-exports the exact retained payload and proposal through a fresh core-owned descriptor. The core commits only a complete pair that passes the same bounds, proposal, length, and digest checks as the original capture. Missing, partial, or changed recovery data remains indeterminate and never becomes a paused checkpoint.

Adapters retain accepted and terminal operation recovery records without age-based expiry. A terminal core commit for an operation known to have been accepted creates a durable release obligation, including when the terminal outcome leaves lifecycle unchanged; only afterward does the core explicitly release the adapter record and durably clear that obligation. Pre-acceptance outcomes create no release obligation. A crash before release completion causes an idempotent retry.

After a complete mutating request has been delivered, a timeout or disconnect without observed acceptance is not treated as safe cancellation: the core sends no termination signal, records acceptance-unknown, and reconciles status. If status reports the operation absent, the core can conclude that the adapter never accepted it only after the daemon that spawned the exact original process has reaped it and durably recorded that proof. A later daemon may trust the committed proof; without it, absence is non-authoritative and the intent remains recovery-required. Losing a status record for a still-pending lifecycle intent after acceptance is instead a protocol violation and leaves durable attention rather than authorizing replay. Once the terminal core commit has replaced that intent with a release obligation, recovery invokes only idempotent release; `released: true`, including for an already absent record, completes the obligation without a status query.

Recovery never parses checkpoint contents, selects a substitute adapter, or blindly repeats a non-idempotent request. An unverifiable result remains visible as attention-required, acceptance-unknown, or indeterminate rather than being reported as completed. Version 1 has no force, assume-failed, forget, replay, or manual resolution command. After an administrator repairs or compatibly upgrades an adapter externally, automatic safe status/release reconciliation can make progress; acknowledgement changes presentation only.

## Checkpoint guarantees

For every accepted checkpoint, Session Manager guarantees:

- the envelope identifies the adapter and checkpoint format;
- stored payload bytes are returned unchanged;
- integrity and configured bounds are checked without semantic parsing;
- core logs and ordinary inspection do not expose payload contents;
- only a compatible adapter receives the payload for resume or required discard;
- stopped-session cleanup follows the defined retention policy;
- missing or incompatible adapters produce explicit outcomes.

Session Manager does not guarantee that a checkpoint is portable, human-readable, editable, or restorable without its producing adapter. The adapter must state any stronger guarantee.

## Public integration behavior

The public API exposes platform-independent data:

- profiles, tasks, roots, ordered desktop-entry IDs, and the fact that each session is one logical workspace group;
- session UUID, generated name, source, number, lifecycle, and active role;
- checkpoint ID, adapter ID, format ID/version, creation time, payload length, preservation/activity qualifiers, and discard requirement, never the internal integrity digest or payload semantics;
- adapter ID/display name, protocol version, availability, format compatibility, and the independent `checkpoint.capture`, `session.suspend`, `checkpoint.restore`, and `checkpoint.discard` capabilities;
- pending operations, stable errors, and attention conditions.

Clients own presentation, search, icons, keyboard bindings, workspace navigation, window lists, application execution, and platform-specific status. Session Manager must remain useful with no graphical client installed.

The public integration surface also exposes the accepted adapter-registry source ETag, a separate adapter-generation revision, and an explicit atomic registry reload operation. External registry edits are not watched or adopted implicitly. Reload cannot replace or remove an adapter referenced by any pending lifecycle intent, accepted or indeterminate operation, or outstanding release obligation.

## Adapter capability vocabulary

An adapter capability is not inferred from its name or checkpoint formats. Compatibility inspection, operation status, and idempotent operation release are mandatory protocol behavior rather than optional capabilities. `checkpoint.capture`, `session.suspend`, `checkpoint.restore`, and `checkpoint.discard` are independent booleans qualified by supported checkpoint format ranges. Version 1 defines no adapter start, switch, window, launcher, or process-control capability.

## Failure behavior

Operations distinguish at least:

- invalid configuration or request;
- stale configuration or runtime precondition;
- unavailable or incompatible adapter;
- unsupported capability;
- invalid or corrupt checkpoint envelope or payload integrity;
- adapter-declared failure;
- safely proven timeout/cancellation before acceptance;
- acceptance-unknown after complete request handoff;
- accepted operation with indeterminate external outcome;
- persistence or recovery failure.

Client timeout does not cancel or signal a handed-off lifecycle operation, whether acceptance is unknown or known. Human-readable messages may explain stable machine-readable codes but do not replace them.

## First-version scope

The first version includes:

- profile and task CRUD;
- root and desktop-entry lists;
- session naming, numbering, active role, and lifecycle;
- generic local CLI and API;
- explicitly configured subprocess-adapter discovery and compatibility;
- opaque checkpoint capture, storage, resume handoff, discard, and recovery;
- distinct application fake-port, fake-subprocess, sample-adapter, and adapter-conformance testing;
- crash-safe core persistence and privacy-safe diagnostics.

The first version does not require:

- a built-in concrete platform, shell, launcher, layout, process, or service-manager integration;
- a built-in compositor, shell, launcher, or process adapter;
- fixed keyboard shortcuts or a fixed workspace count;
- a core-visible workspace slot, standard/special/global workspace role, or task window destination;
- window discovery, ownership, correlation, movement, placement, focus, or closure in the core;
- process launch, attachment, suspension, signalling, or termination in the core;
- a universal semantic window or layout checkpoint schema;
- checkpoint portability between adapters;
- security isolation between sessions;
- remote or multi-user operation.

## Functional acceptance

The MVP is functionally accepted when:

1. users can manage valid profiles and tasks without any desktop component installed;
2. concurrent sessions follow the documented identity, naming, numbering, rename, and deletion rules;
3. start, switch, pause, resume, stop, and recovery expose truthful core and adapter outcomes;
4. opaque payload bytes survive storage, daemon restart, and adapter handoff unchanged;
5. an incompatible or missing adapter cannot receive or reinterpret a checkpoint;
6. clients can build integrations from the generic API without platform-specific core methods;
7. the complete default test suite cannot discover or mutate the live desktop.
