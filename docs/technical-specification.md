# Technical Specification

## Purpose and status

This document is the accepted authoritative technical contract for the platform-independent Session Manager core. It implements the [functional specification](functional-specification.md) and [ADR 0010](decisions/0010-keep-the-core-platform-independent-with-opaque-adapter-checkpoints.md).

The platform-independent architecture and first-version contracts are settled. Concrete adapters and clients may add platform behavior only outside the core schema and method inventory.

## Design constraints

- The core imports no compositor, layout, shell, window-system, launcher, process-manager, or desktop-integration library.
- Concrete integrations depend on generic core contracts; the dependency never points from the core to a concrete integration.
- Checkpoint payload bytes are opaque and immutable from the core's perspective.
- Platform effects occur only through the generic adapter port or outside Session Manager through an API client.
- One local daemon owns persistent state and serializes conflicting mutations.
- Core operation results are durable, versioned, bounded, privacy-safe, and truthful about unknown adapter outcomes.
- Default tests cannot discover or invoke installed adapters or the live desktop.

## Architecture

```text
CLI and arbitrary local clients
              |
              v
       versioned public API
              |
              v
  application and domain core
       |                 |
       v                 v
storage ports     generic adapter port
                         ^
                         |
                concrete adapters
```

The core contains:

- domain types and invariants;
- profile, task, session, naming, and lifecycle application services;
- lossless configuration editing;
- runtime, checkpoint, transaction, and recovery storage;
- generic adapter coordination;
- local IPC and CLI surfaces;
- sanitized diagnostics.

It contains no platform adapter. A client may react to core state without being an adapter. For example, a shell can render sessions and a compositor configuration can map shortcuts to API requests. An adapter is used only when Session Manager delegates a lifecycle effect and receives a contract-defined result.

## Implementation language and project structure

The core uses stable Rust with the 2024 edition in one Cargo package, one library target, and one `session-manager` executable. Internal modules follow this dependency direction:

```text
interfaces/ -> application/ -> domain/
adapters/storage/ -----------^
adapter_contract/ -----------^
```

`adapter_contract` contains only platform-neutral request, response, capability, compatibility, and opaque-payload types. A production concrete adapter is not linked into the core package. Version 1 invokes explicitly configured adapters as subprocesses through the protocol defined below; application code still depends only on a typed port.

## Domain model

### Profile

A profile has a unique name, root path value, ordered unique desktop-entry IDs, and ordered tasks. A task has a name unique within its profile and an ordered unique desktop-entry list restricted to entries in that profile. Empty profile and task desktop-entry lists are valid. Profile name and profile-plus-task name are the configuration identities in version 1.

Profile and task names are logical keys, never file or adapter identifiers. Desktop-entry IDs are data references; the core validates their syntax but does not resolve or execute desktop files. Configuration contains no workspace slots or destinations.

### Session

A persisted live session contains:

- stable UUID;
- source profile name and optional task name;
- current source labels and generated name;
- positive 32-bit per-source number;
- lifecycle `running` or `paused`;
- active-role flag derived from the top-level active session ID;
- immutable start snapshot of platform-independent source data;
- logical workspace-group ID equal to the session UUID;
- selected checkpoint-adapter identity only after pause begins;
- checkpoint reference only while paused;
- current attention and recovery metadata.

Stopped sessions are removed after completion. Start allocation runs under the serialized mutation lock and atomically commits the new session with its direct-final public operation. Candidate transaction files are not accepted runtime state, never reserve a name or number, and never participate in rename or deletion guards. Runtime identity never derives from display names, paths, client IDs, or adapter-private references.

One session represents one logical workspace group. The group has no separate persisted object, member list, fixed count, standard/special/global role, or concrete workspace reference. Clients and adapters use the session UUID as the correlation key for their own mapping.

### Naming and rename transactions

Profile sessions use `<profile-name>-<number>` and task sessions use `<profile-name>-<task-name>-<number>`. Generated names are globally unique across running and paused sessions even when profile/task delimiter combinations would otherwise produce the same text. Running and paused sessions reserve numbers; stopped sessions and uncommitted transaction candidates do not.

Numbers range from 1 through 4,294,967,295 independently for each launch source. Under the mutation lock, allocation chooses `1` when the source has no live session and otherwise one more than its highest running or paused number. If that highest number is 4,294,967,295, allocation fails with `conflict.session_number_exhausted` without changing state; lower gaps are not reused while a higher live number remains. After calculating the number and name but before the atomic start commit, start rejects a name already held by another source with `conflict.session_name_collision`; it does not skip to another number or mutate state.

Explicit profile or task rename validates the complete candidate name set and updates configuration and every affected pending/live label atomically. It preserves UUIDs, source association, number, lifecycle, adapter identity, checkpoint reference, envelope, and payload bytes.

### Lifecycle invariants

- at most one session is active;
- only a running session may be active;
- paused sessions have exactly one committed checkpoint reference;
- running sessions have no committed paused checkpoint;
- a completed stopped session has no runtime record or retained payload;
- an adapter outcome cannot be promoted to a stronger core outcome than the generic contract permits;
- an acceptance-unknown or indeterminate non-idempotent operation remains recoverable and is never replayed automatically.

## Configuration

### Location and format

Configuration is versioned UTF-8 TOML 1.0 at:

```text
$XDG_CONFIG_HOME/session-manager/config.toml
```

with the XDG fallback `$HOME/.config/session-manager/config.toml`.

The schema contains core-owned profile, task, desktop-entry, and root data only. Tasks contain only their name and desktop-entry IDs. The logical workspace group is created with the session and therefore has no configuration table. Configuration contains no concrete workspace names, fixed roles, global/special workspace, task window destination, layout placement, shell model, keyboard binding, window selector, command, PID, process unit, adapter selection, or checkpoint payload.

```toml
schema_version = 1

[[profiles]]
name = "FDS"
root = "/home/user/work/fds"
desktop_entries = [
  "org.kde.kdenlive.desktop",
  "org.mozilla.firefox.desktop",
]

[[profiles.tasks]]
name = "MontageVidéo"
desktop_entries = ["org.kde.kdenlive.desktop"]

[[profiles.tasks]]
name = "MiniaMaking"
desktop_entries = []
```

`root` is an absolute lexically normalized UTF-8 path of at most 4,096 bytes: it starts with `/`, contains no NUL, empty component, `.` component, or `..` component, and removes no symlink or filesystem alias through canonicalization. The root `/` is valid. Existence and symlink resolution are reported separately and are not required for read-only configuration inspection. Desktop-entry IDs are 1–255 byte UTF-8 strings ending in `.desktop`, contain no slash or NUL, and are compared byte-for-byte. Profile entries and each task subset are ordered and unique. Session Manager does not resolve the IDs against XDG application directories.

The exact configuration file is at most 1,048,576 bytes. It contains at most 256 profiles, 256 tasks per profile, and 1,024 desktop-entry IDs per profile; a task cannot exceed its parent list. Profile and task names are 1–128 byte UTF-8 strings without NUL or ASCII control characters. These bounds are validated before semantic decoding and guarantee that one complete public configuration projection fits within the IPC logical-message limit.

### Validation and format preservation

The final schema uses strict versioned decoding with unknown-field rejection, stable field paths, unique logical names, and referential validation. Zero `[[profiles]]` tables is the empty-profile representation; `profiles = []` is rejected.

The daemon retains the exact accepted bytes and a typed validated model. Programmatic mutations use a lossless TOML syntax document and change only the smallest unambiguous syntactic boundary. Unrelated comments, whitespace, quoting, ordering, and line endings remain byte-for-byte identical. A candidate is reparsed and fully validated before commit. Ambiguous edits fail rather than serializing the whole typed model.

Before mutation, the daemon compares the current file bytes and identity with the last accepted document both before candidate creation and under the commit lock. Divergence returns `conflict.configuration_changed_on_disk`. Only explicit reload adopts externally edited bytes.

## Persistent runtime and checkpoint storage

### Locations

Default paths are:

| Purpose | Path |
| --- | --- |
| Adapter registry | `$XDG_CONFIG_HOME/session-manager/adapters.toml` |
| Runtime metadata | `$XDG_STATE_HOME/session-manager/runtime.json` |
| Previous runtime | `$XDG_STATE_HOME/session-manager/runtime.previous.json` |
| Checkpoint payloads | `$XDG_STATE_HOME/session-manager/checkpoints/` |
| Transaction manifests | `$XDG_STATE_HOME/session-manager/transactions/` |
| Runtime lock | `$XDG_RUNTIME_DIR/session-manager/state.lock` |
| IPC socket | `$XDG_RUNTIME_DIR/session-manager/ipc-v1.sock` |

State directories are mode `0700`; core-created files are mode `0600`. Exact checkpoint filenames derive only from UUIDs, never user or adapter labels.

### Runtime metadata

`runtime.json` is strict versioned UTF-8 JSON containing core revision, shutdown marker, `max_observed_unix_ms`, active session ID, live sessions, attention, recovery state, pending lifecycle operations, pending adapter-release obligations, and bounded public operation history. A delivered pending adapter intent records `handoff_complete` and may record `original_invocation_exit_observed`; only the daemon instance that spawned and reaped that exact child may set the latter, in a durable runtime commit. A paused session references a checkpoint UUID and envelope digest. Runtime does not embed a parsed adapter state model.

When an adapter operation has durably reached `accepted`, its terminal core commit atomically replaces the pending lifecycle intent with a release obligation containing the adapter ID and core operation UUID. This applies to every accepted terminal outcome, including one that leaves the session lifecycle unchanged. Core-only operations and adapter outcomes returned before acceptance create no release obligation. Adapter release occurs only after the terminal commit. A completed `operation.release` removes the obligation in a later durable runtime commit. Startup and steady-state recovery retry release obligations idempotently until completion; a crash at any boundary can therefore lose neither recovery authority nor the duty to release adapter-owned data.

Pending operations and release obligations are never removed by history retention. Public terminal operation records are retained for 30 days and at most 1,000 records; cleanup removes the oldest terminal records first when either bound is exceeded. Cleanup never removes a pending operation, release obligation, referenced attention condition, or the only evidence needed for recovery. Operation history and adapter release are independent: releasing adapter data does not immediately remove the public terminal record.

After a terminal history record is removed, `operation.get` returns `not_found.operation`; this has no effect on session state, attention, checkpoint retention, or adapter-release completion. Runtime metadata durably stores `max_observed_unix_ms`. On every load and commit, the effective retention time is `max(stored max_observed_unix_ms, current injected wall-clock time)`, and the resulting maximum is persisted on the next commit. Age is measured against that effective time, so a backward clock adjustment cannot extend retention; a forward jump may expire records immediately. Record ordering uses committed runtime revision and operation UUID as a deterministic tie-breaker and never uses wall-clock order.

### Public operation records

Version 1 creates public operation records only for admitted `session.start`, `session.switch`, `session.pause`, `session.resume`, and `session.stop` requests. Configuration, adapter-registry, attention, validation, inspection, and release-control methods are synchronous mutations or queries and create no public operation record. Admission occurs only after strict request decoding, peer authorization, required preconditions, target/source existence, lifecycle guards, operation-specific naming checks, operation-specific adapter availability and capability/format compatibility preflight, and capacity reservation have succeeded. Start, switch, running stop, and paused stop without adapter discard perform no adapter preflight. Pause validates capture requirements; resume validates restore requirements; paused stop with required discard validates discard requirements. A lifecycle request rejected by those checks returns a synchronous error and creates no lifecycle operation UUID, history record, or lifecycle event; an explicit adapter reload or other independent mutation remains governed by its own event rules. Admission then assigns the operation UUID and target session UUID before the first lifecycle mutation. The admitting durable commit creates the pending intent for an adapter-backed operation or the direct-final operation and state change for a core-only operation, so a crash cannot expose a public operation without its corresponding durable authority. For start, allocation, UUID assignment, session creation, and the direct-final operation are one commit; there is no accepted pending start record.

The exact public operation fields are:

- `operation_id`: operation UUID;
- `kind`: one of `session.start`, `session.switch`, `session.pause`, `session.resume`, or `session.stop`;
- `target_session_id`: UUID retained as historical correlation even after that session stops;
- `phase`: one of `pending`, `acceptance_unknown`, `accepted`, `indeterminate`, or `final`;
- `outcome`: absent unless `phase = final`, then one of `completed`, `completed_with_limitations`, `unsupported`, `rejected`, `failed`, or `cancelled`;
- `error_code`: optional stable code describing a final transport/core failure or the current non-final recovery condition; it never replaces `outcome`;
- `created_at_unix_ms`: required;
- `accepted_at_unix_ms`: present only after acceptance is known, including acceptance recovered by status;
- `finished_at_unix_ms`: present only when `phase = final`;
- `last_updated_revision`: the runtime revision that last changed the record.

Legal phase transitions are `pending -> final` for core-only operations, known pre-handoff failures, and explicit pre-acceptance finals; `pending -> acceptance_unknown -> accepted|indeterminate|final`; `pending -> accepted -> indeterminate|final`; and `accepted|indeterminate -> final`. Status may resolve `acceptance_unknown` directly to `accepted`, `indeterminate`, or `final`. No phase moves backward. Core-only start, switch, and running stop commit directly as `final/completed`. Local paused stop without adapter discard also commits directly as `final/completed`. `completed_with_limitations` is valid only for resume. A transport/core failure that becomes safely final uses `outcome = failed` plus its stable `error_code`.

Historical correlation UUIDs are not live referential-integrity edges. A terminal operation may target a stopped session, and an adapter-release obligation may outlive the session or checkpoint whose accepted operation created it. Every pending lifecycle intent must reference the live session required by its transition; start is core-only and never has a pending intent. Runtime validation must preserve these historical records while rejecting a pending intent whose required live target is missing.

Runtime state contains at most 1,024 live sessions, 1,024 combined pending lifecycle operations and release obligations, 1,000 terminal history records, and 4,096 attention records, and its exact JSON file is at most 8,388,608 bytes. Repeated identical attention keys are coalesced. Before any external effect, the durable intent reserves worst-case space for its terminal history entry, release obligation, attention outcome, and exact combined public snapshot/event encoding. A prospective mutation that cannot reserve storage or fit both public encodings within 16 MiB fails with `recovery.capacity_exceeded`. The transport-neutral projection encoder and size check are persistence/application invariants reused by IPC, not a late socket-only check. Recovery therefore replaces already reserved records without exceeding a bound or creating unpublishable valid state. An externally edited or corrupt over-bound runtime file is retained unchanged under the read-only recovery gate and is never partially loaded or rewritten.

Runtime inspection may expose safe envelope metadata but never payload bytes, adapter-private operation details, or external desktop state.

### Checkpoint envelope

The core-owned UTF-8 JSON envelope has this version 1 shape:

```json
{
  "envelope_version": 1,
  "checkpoint_id": "01900000-0000-7000-8000-000000000001",
  "session_id": "01900000-0000-7000-8000-000000000002",
  "adapter_id": "org.example.adapter",
  "adapter_protocol_version": 1,
  "format_id": "org.example.session",
  "format_version": 3,
  "created_at_unix_ms": 1784246400000,
  "payload": {
    "media_type": "application/octet-stream",
    "length": 1234,
    "sha256": "64-lowercase-hex-characters"
  },
  "preservation": "snapshot",
  "activity": "unchanged",
  "restore_expectation": "best_effort",
  "discard_requires_adapter": false
}
```

All fields are required. `checkpoint_id` and `session_id` are canonical lowercase hyphenated UUIDs. `adapter_id` and `format_id` use the machine-identifier grammar: 3–128 lowercase ASCII bytes, dot-separated segments that start and end with an alphanumeric character and otherwise contain lowercase alphanumerics or hyphens. `format_version` and `adapter_protocol_version` are positive 32-bit integers. `media_type` is an ASCII MIME type of at most 127 bytes and is descriptive only; the core performs no content conversion. Timestamps are diagnostic and never select revisions.

The core assigns checkpoint/session IDs, creation time, payload length, and digest. The adapter binds its result to the accepted operation identity and supplies adapter ID, protocol version, format ID/version, media type, preservation, activity, restore expectation, and discard requirement. The core validates and assembles the final envelope; neither a client nor adapter may override core-assigned fields.

`preservation` is `snapshot` or `live`; `activity` is `unchanged` or `suspended`; `restore_expectation` is `exact` or `best_effort`. The core displays these adapter assertions but does not independently prove platform semantics. `activity = suspended` requires the adapter to declare `session.suspend`; `preservation = live` requires adapter-assisted discard.

Payload length is from 0 through 67,108,864 bytes (64 MiB). The envelope is at most 16 KiB. SHA-256 is calculated over the exact untransformed payload bytes. Version 1 performs no compression or encryption transform. Payload and envelope are confidential user state: mode `0600`, excluded from ordinary API bodies and logs, and never copied into diagnostics or CI artifacts.

An adapter or format mismatch fails before payload handoff. Version 1 exposes no migration operation. Any future migration is owned by the adapter, must produce a new envelope and payload, and must preserve the old pair until atomic replacement commits; the core never derives a migrated payload itself.

Paused checkpoints are retained until successful resume or stop. Successful resume removes the stored checkpoint only after the running-state commit. Stop follows `discard_requires_adapter`; failed, acceptance-unknown, or indeterminate required discard retains the checkpoint. The core exposes no age-based or startup-time garbage collection of referenced checkpoints.

### Opaque payload invariant

The storage layer accepts payload bytes only through the adapter coordination boundary and enforces generic size and integrity rules. Once accepted, the same byte sequence is used for digest verification, restart recovery, resume handoff, and discard handoff.

The core never:

- deserializes payload contents;
- converts text encoding or line endings;
- canonicalizes, compresses, decompresses, or encrypts without an envelope-declared generic storage transform whose inverse preserves exact adapter bytes;
- combines payloads;
- edits a payload in place;
- synthesizes a replacement from profile or runtime fields;
- lets a client relabel an existing payload as another adapter or format.

Version 1 never migrates a payload. Compatibility is satisfied only when the registered adapter directly supports the stored format ID and version.

### Atomicity

Each single file is written through a unique same-directory candidate, file synchronization, atomic replacement, and parent-directory synchronization.

Operations spanning configuration, runtime metadata, or checkpoint files use a durable transaction manifest containing only core identifiers, old/new revisions and ETags, candidate paths, digests, and intended commit order. Recovery selects a complete validated old or new set and never reconstructs configuration presentation or opaque payload bytes.

Payload deletion occurs only after the runtime commit no longer references it and the final lifecycle contract permits discard. Unreferenced candidates are quarantined or removed only when ownership and transaction status are unambiguous.

## Generic adapter contract

### Responsibilities

The core contract defines platform-neutral requests and outcomes. A concrete adapter exclusively owns:

- discovery and validation of the platform it integrates;
- interpretation of the session UUID, root, and ordered desktop-entry snapshot supplied for checkpoint capture;
- every platform-specific external effect;
- checkpoint payload schema and serialization;
- capture consistency and any live-resource suspension;
- payload compatibility and any future migration schema;
- restoration and its verification;
- cleanup or discard effects;
- adapter-specific diagnostics that do not expose payloads through core logs.

The core owns adapter selection, request validation, generic deadlines, durable intent, payload storage, lifecycle transition, and publication of the verified generic outcome.

### Identity and compatibility

Adapter identity and checkpoint-format identity are stable machine identifiers, not display names or executable paths. A resume or discard request is routed only when the discovered adapter proves compatibility with the stored envelope under the final compatibility rules.

The core never chooses an adapter by fuzzy match, rewrites an envelope to satisfy discovery, or falls back to another adapter. Version 1 adapter upgrades must retain direct compatibility or report the checkpoint unsupported; future migration requires a new contract version.

`hello` reports exact adapter ID, optional display name, protocol version `1`, capabilities, and supported format ranges. `compatibility.inspect` returns only `compatible` or `incompatible` plus a stable bounded reason code after examining envelope metadata; the payload is not provided for inspection. Both the hello range and explicit inspection must accept the stored format before payload handoff. Version 1 has no downgrade or migration result.

### Capabilities

Compatibility inspection, operation-status lookup, and operation release are mandatory protocol methods, not capabilities. Version 1 defines four independent capability IDs:

- `checkpoint.capture`: create one envelope proposal and opaque payload for a running session;
- `session.suspend`: permit capture to report verified `activity = suspended`;
- `checkpoint.restore`: consume a compatible payload and restore or resume adapter-owned state;
- `checkpoint.discard`: release adapter-owned live state or perform adapter-required checkpoint disposal.

`hello.capabilities` is an ordered list unique by `capability_id`. Each object contains `capability_id` and its own ordered `formats`, with at most one entry per `format_id`; each format contains `format_id`, `min_version`, and `max_version`, with `min_version <= max_version`. Duplicate capability IDs, duplicate format IDs within one capability, and invalid ranges are protocol errors. An empty `formats` list means the capability is format-independent. A non-empty list restricts that capability to matching format IDs and inclusive versions. Version 1 has no adapter start, switch, workspace, window, launcher, or generic process-control capability. A format advertised for one capability implies nothing about another capability.

Capabilities are independent; requiring several for one safe transition does not make their advertisements imply one another. Missing optional behavior disables only the lifecycle transition that requires it. Capability presence is not proof of operation success; every mutation returns a separate outcome. Capture has no caller-selected format in version 1: the adapter deterministically chooses one proposal for the operation UUID, and every completed response or re-export for that UUID must repeat it exactly.

Pause preflight requires at least one format/version intersection between `checkpoint.capture` and `checkpoint.restore`, treating an empty range list as format-independent. Before committing a completed capture, the core verifies that the chosen format/version matches both capabilities. This guarantees that a newly paused checkpoint is restorable by construction, while a later incompatible upgrade can still make it explicitly unavailable. If the result reports `activity = suspended`, the adapter must also advertise `session.suspend`, and the chosen format/version must match its non-empty ranges; empty suspend ranges apply format-independently. If `discard_requires_adapter = true`, the chosen format/version must likewise match `checkpoint.discard`; `preservation = live` requires `discard_requires_adapter = true`. A proposal that violates any of these rules is an invalid adapter result: it never commits paused, remains recoverable if already accepted, and follows the accepted terminal release rules after the core records failure.

### Requests and outcomes

Every mutating request carries a core operation UUID, session UUID, accepted base revision, adapter identity, bounded platform-independent context, and, when applicable, an integrity-verified opaque payload. The core assigns the operation UUID and persists the complete intent before invoking the adapter. The operation UUID is the mandatory durable recovery key and an adapter must make status queryable by that UUID before emitting `accepted` or performing its first external effect.

Final outcomes are `completed`, `completed_with_limitations`, `unsupported`, `rejected`, `failed`, and `cancelled`. Failure to spawn the adapter or deliver one complete valid request line is a known pre-handoff failure and does not change lifecycle. Once the complete mutating request has been delivered, transport loss or deadline expiry without an observed `accepted` is acceptance-unknown: the adapter may already have durably accepted the operation, so the core retains the intent and reconciles by status without replay. Loss or expiry after known acceptance is `indeterminate` until mandatory `operation.status` returns a final outcome. The adapter may supply a 1–128 byte opaque ASCII operation token at acceptance as a secondary correlation value; the core stores but never interprets it. Recovery never depends on receiving or retaining that token.

`completed_with_limitations` is permitted for restore and records an attention condition while transitioning to running. Capture must be `completed` to commit paused. Discard must be `completed` to remove adapter-required state. `unsupported`, `rejected`, `failed`, and `cancelled` never advance lifecycle.

The adapter contract must define idempotency per method. Automatic recovery may repeat read-only status or an explicitly idempotent desired-state request with the same operation identity. It never repeats a non-idempotent request whose outcome is unknown.

### Transport boundary

Application code depends on an in-memory typed port. Version 1 implements that port with explicitly configured, one-operation-per-invocation subprocess adapters as recorded in [ADR 0011](decisions/0011-use-explicit-subprocess-adapters.md).

Adapter registrations live in strict mode-`0600` UTF-8 TOML owned by the daemon user at `$XDG_CONFIG_HOME/session-manager/adapters.toml`. Each entry contains an adapter ID, canonical absolute executable path, and optional ordered unique `pass_environment` names. Environment names use an uppercase ASCII identifier grammar and values are read only at invocation. There is no argument string, shell expansion, PATH lookup, directory scanning, plugin loading, or executable download. Duplicate IDs, relative paths, symlinks, non-regular files, executables owned by neither the daemon user nor root, group/world-writable executables, forbidden environment names, and an executable whose reported ID differs from its registration are rejected.

The exact adapter registry is at most 262,144 bytes and contains at most 128 adapters. Each adapter has at most 64 forwarded environment names; each name is 1–127 ASCII bytes. At invocation, each forwarded value must be valid UTF-8 without NUL and at most 4,096 bytes or that adapter invocation fails before spawn as unavailable. Executable paths follow the root path byte and lexical-component bounds. The registry source ETag is lowercase hexadecimal SHA-256 of the exact accepted bytes; a missing registry uses the SHA-256 of the empty byte sequence as its source ETag.

```toml
schema_version = 1

[[adapters]]
id = "org.example.session-adapter"
executable = "/usr/libexec/example-session-adapter"
pass_environment = ["WAYLAND_DISPLAY"]
```

The registry order is presentation-only and never selects a default adapter. Pause must name one adapter explicitly; resume and required discard use the envelope's exact adapter ID.

A missing registry means zero available adapters and does not prevent profile, task, start, switch, running stop, or inspection operations. A registry with invalid TOML, duplicate or mismatched identities, unsafe registration data, or an invalid hello result is rejected as a whole and is never partially accepted; it does not invalidate `config.toml` or existing checkpoints. An otherwise valid registered adapter whose executable cannot be opened or whose bounded hello invocation fails or times out is accepted with `availability: unavailable` and a bounded reason, allowing a later explicit reload to retry validation without changing source bytes.

The daemon loads and validates the registry at startup and retains its exact accepted bytes, file identity, SHA-256 source ETag, and complete accepted public adapter projection. It assigns the accepted source/projection pair a canonical UUID `adapter_generation`. The ETag remains exclusively the digest of the source bytes and is the on-disk reload precondition; the generation is the public revision token for the accepted registry state, including validated hello metadata and availability. Startup assigns a fresh generation. A successful explicit reload assigns a new generation and publishes `adapter_registry.changed` when either the accepted source ETag or public projection differs. Only an exact source-and-projection no-op preserves the generation and publishes no event. This means a comment-only source edit changes both ETag and generation, while repeated validation whose availability changes may change the generation even when the source ETag does not. It never watches or automatically adopts external changes. `adapter.reload` is the only runtime operation that may adopt a changed accepted registry state: it compares the current file with the last accepted identity and ETag, validates the complete candidate registry, and atomically publishes either the whole new source/projection pair or no change. A missing candidate means an empty registry. An invalid, concurrently changed, or partially readable candidate fails without changing either token or the accepted projection.

A reload may add adapters while operations are pending. It must reject changing the executable path or identity-sensitive registration fields of, or removing, any adapter referenced by a pending lifecycle intent in any phase, an accepted or indeterminate operation, or an outstanding adapter-release obligation. Once none of those records references the adapter, a later explicit reload may change or remove it. Each adapter invocation uses one immutable accepted registry generation; a reload never changes an already spawned invocation.

A registry generation pins registration data and accepted hello metadata, not the executable's bytes across separate invocations. Adapter ID, protocol version, display name, capabilities, and format ranges must be deterministic for one executable identity and accepted registry generation; operational availability is the result of the last startup or explicit-reload validation, not a live health signal. Transient invocation failures are reported through that request and attention without silently changing the adapter projection. An administrator may atomically replace the file at the registered path; concurrently modifying the bytes of an already opened inode is unsupported and causes the spawn to fail when detected. Each requested control invocation first opens the registered path without following symlinks and validates that opened regular file's identity, ownership, and permissions. File identity comprises device, inode, nanosecond change time, size, mode, owner, and group from the opened descriptor and is rechecked immediately before execution; any change fails before request handoff. Hello validation is cached only for that complete identity together with the accepted adapter generation. If the identity/generation pair is not cached, the core duplicates the same opened descriptor and invokes `hello` from one descriptor. The requested method may run from the other descriptor only if ID, protocol version, display name, capabilities, and format ranges exactly equal the accepted projection for that registration; otherwise the invocation fails as unavailable without adopting new metadata, and an explicit reload is required. A cached identity/generation pair may execute the requested method directly from its opened descriptor. Descriptor-based execution uses a facility such as Linux `execveat` with `AT_EMPTY_PATH`; the core never validates one inode and then executes a fresh path lookup. Failure to validate or execute that exact image fails before request handoff. A compatible atomic replacement must also preserve status lookup, capture re-export, and release for every unreleased operation.

The daemon invokes:

```text
ADAPTER_EXECUTABLE session-manager-adapter --protocol 1 --method METHOD
```

No shell is involved. Standard input and output carry bounded JSON Lines control envelopes. A lifecycle-mutating method either returns one permitted pre-acceptance final result without external effect, or emits exactly one `accepted` envelope before external mutation and then exactly one final envelope. Control methods `hello`, `compatibility.inspect`, `operation.status`, and `operation.release` return one final envelope without `accepted`. Unknown fields and enum values fail protocol validation.

### Subprocess protocol version 1

The only valid `METHOD` values are:

```text
hello
compatibility.inspect
operation.status
operation.release
checkpoint.capture
checkpoint.restore
checkpoint.discard
```

`session.suspend` is a capability asserted by a successful capture result, not a separately invokable method. Each invocation consumes exactly one request line on standard input. Every control object contains `protocol_version: 1` and rejects duplicate keys, unknown fields, trailing non-whitespace data, and invalid UTF-8. UUID fields use canonical lowercase hyphenated UUID text. Unsigned integers use JSON integers and must fit their documented width.

Protocol version 1 uses these additional normative bounds:

| Value | Limit |
| --- | ---: |
| Adapter display name | 256 UTF-8 bytes |
| Capabilities per adapter | 4 |
| Format ranges per capability | 64 |
| Desktop-entry IDs in capture context | 1,024 |
| Root | 4,096 UTF-8 bytes |
| Stable code, reason code, or field path | 128 ASCII bytes |
| Human diagnostic message | 1,024 UTF-8 bytes |
| Restore limitation codes | 64 |
| Operation token | 128 ASCII bytes |

Machine IDs, desktop-entry IDs, paths, versions, envelope size, payload size, control-line size, complete control-message size, and stderr use the bounds defined in their owning sections. Exceeding any bound is `protocol.invalid_message` and causes no core lifecycle transition.

Every request uses this envelope:

```json
{
  "protocol_version": 1,
  "request_id": "UUID",
  "method": "checkpoint.capture",
  "params": {}
}
```

`request_id` correlates only the subprocess exchange. For lifecycle-mutating methods, `params.operation_id` is the core-assigned durable UUID and `params.session_id`, `params.base_revision`, and `params.adapter_id` are required. `base_revision` is an unsigned 64-bit integer. `checkpoint.capture` additionally receives `root`, ordered `desktop_entries`, `output_fd`, and `deadline_unix_ms`. `checkpoint.restore` and `checkpoint.discard` receive `checkpoint`, `input_fd`, and `deadline_unix_ms`; `checkpoint` is the complete core envelope without payload bytes. Descriptor values are non-negative inherited descriptor numbers, are present only for their documented method, and are never inferred from a fixed number.

`hello` has empty params. `compatibility.inspect` receives `adapter_id` and checkpoint envelope metadata without a descriptor or payload. `operation.status` receives `adapter_id` and the core `operation_id`; it may additionally carry a previously stored `operation_token`, but the token is never required to locate the operation. When recovering a completed capture whose core candidate is not durable, status also receives a fresh `output_fd`; otherwise that field is forbidden. `operation.release` receives `adapter_id` and the target `operation_id`; it is an idempotent control request that returns one final result without `accepted` and performs no platform lifecycle effect.

An accepted mutating invocation first returns:

```json
{
  "protocol_version": 1,
  "request_id": "UUID",
  "type": "accepted",
  "operation_id": "UUID",
  "operation_token": "optional-adapter-token"
}
```

Before writing `accepted`, the adapter must durably register `operation_id` as `accepted` in adapter-owned state. It must return the same logical operation for repeated status queries and must reject reuse of an operation UUID with different parameters. Alternatively, before registration, acceptance, and any external effect, it may return exactly one final lifecycle result with outcome `unsupported`, `rejected`, `failed`, or `cancelled`. A pre-acceptance final `completed` or `completed_with_limitations` is invalid. A final response has `type: "result"`, repeats `request_id` and, for mutations, `operation_id`, and contains exactly one of `result` or `error`.

Successful read-only results have these exact logical fields:

- `hello`: `adapter_id`, optional `display_name`, `protocol_version`, and ordered capabilities unique by `capability_id`, each containing ordered formats unique by `format_id` with one valid inclusive `min_version`/`max_version` range;
- `compatibility.inspect`: `compatibility` equal to `compatible` or `incompatible`, plus optional stable `reason_code` for the incompatible result;
- `operation.status`: `status` equal to `accepted`, `completed`, `completed_with_limitations`, `unsupported`, `rejected`, `failed`, or `cancelled`, optional `operation_token`, and the same method-specific final data that the original invocation would return when terminal.

Lifecycle-mutation results contain `outcome` with the same terminal vocabulary. A completed capture additionally contains `checkpoint_proposal` with `format_id`, `format_version`, `media_type`, `preservation`, `activity`, `restore_expectation`, and `discard_requires_adapter`. Restore may include a bounded ordered `limitations` list of stable codes only when its outcome is `completed_with_limitations`. Discard has no method-specific result fields. Capture cannot return `completed_with_limitations`. Successful `operation.release` returns `released: true`.

An error object contains `code`, optional bounded `message`, and optional `field`; codes are stable lowercase dot-separated ASCII identifiers. Protocol/framing errors use `protocol.invalid_request`, `protocol.unsupported_version`, `protocol.unknown_method`, `protocol.invalid_message`, `protocol.message_too_large`, or `protocol.internal`. Human messages are diagnostic and never used for branching. An adapter-declared lifecycle failure is a result outcome, not a protocol error.

The adapter must durably retain every accepted status, terminal result, and method-specific recovery data across process exit, adapter restart, and compatible adapter upgrade. It must not expire or forget them based on age. After the core has durably committed the terminal result of an accepted adapter operation, it atomically replaces the lifecycle intent with a release obligation and never invokes `operation.status` for that operation again. It invokes only `operation.release` until the obligation completes. Release deletes the retained record; releasing an already absent operation still returns `released: true`, so a lost release response is safely retried without a permanent tombstone. The core removes the obligation only after receiving `released: true` and durably committing that completion.

`operation.status` is used only while a lifecycle intent remains pending. Its `not_found.operation` result has phase-dependent meaning. When the core has delivered the request but has no durable evidence that `accepted` was observed, absence proves non-acceptance only after the exact original subprocess has been reaped and that fact is durably recorded as `original_invocation_exit_observed = true`. Only the daemon instance that spawned and reaped the child may create this proof, but a later daemon may trust the committed proof. The core may then resolve the attempt as a known pre-acceptance failure and clear the intent without a release obligation, because registration is required before every external effect. While the original process may still be alive, or after a daemon restart with no durable exit proof, `not_found.operation` is non-authoritative and the intent remains acceptance-unknown/recovery-required; the core neither clears it nor invokes the mutation again. A later found accepted/terminal record may still resolve it. If the pending intent has recorded acceptance, `not_found.operation` is a protocol violation: the core retains an indeterminate/attention state and never guesses or replays the mutation. A terminal accepted result is committed by replacing the intent with a release obligation, after which status is forbidden and already-absent release is a successful idempotent completion. A pre-acceptance final response creates no adapter record and no release obligation.

For a completed capture, retained recovery data includes the exact checkpoint proposal and payload bytes. When reconciling a capture whose core-owned candidate was not durably committed, the core supplies a fresh `output_fd` to `operation.status`. A terminal completed status must re-export the identical payload to that descriptor and return the identical proposal. Re-export is read-only and idempotent. The core recomputes length and SHA-256, validates the proposal, and commits only the complete verified pair. A partial, missing, changed, oversized, or unverifiable re-export never becomes authoritative; the session remains running and recovery-required. An adapter that cannot durably retain and reproduce a completed capture must report the original capture as `failed`, never `completed`.

The adapter must persist terminal status and all method-specific recovery data before attempting to write the final stdout envelope. Closing stdout, `EPIPE`, child exit, or loss of the original descriptor therefore cannot erase the recoverable result. Concurrent invocations for different operation UUIDs are permitted; an adapter must either support them safely or serialize them internally without changing protocol outcomes.

Opaque payload bytes never travel inside JSON. For capture, the adapter writes them to an inherited core-created output file descriptor; for restore and discard it receives a read-only inherited input descriptor positioned at byte zero. The core verifies exact length and SHA-256 after capture and before every handoff. Adapters receive no checkpoint filesystem path.

Control messages and their single JSON line are at most 1,048,576 bytes, sufficient for the maximum bounded capture context. The default deadline is 30 seconds for capture, restore, discard, and `operation.status` when it carries a capture `output_fd`; it is 5 seconds for hello, compatibility, metadata-only status, and release. The core enforces elapsed deadlines with a monotonic clock; `deadline_unix_ms` is adapter-visible diagnostic/context data and is not the core's timeout authority.

The request handoff boundary is the successful write of the complete request line. Before that boundary, the adapter cannot decode a valid request and the core may close descriptors and terminate and reap the child. After that boundary, the core sends neither `SIGTERM` nor `SIGKILL` for a lifecycle mutation, even if `accepted` has not yet been observed, because durable acceptance and observation of the stdout envelope can race. On timeout or transport loss it closes its control and payload descriptors as applicable, records `acceptance_unknown` or `indeterminate`, tracks and asynchronously reaps the exact child, durably records exact-child exit before relying on it, and reconciles through a new status invocation. Until that proof commits, missing status cannot prove non-acceptance. An uncooperative child may therefore keep the intent blocked and visible rather than being unsafely terminated. Child exit alone never proves that an accepted operation failed.

The subprocess receives `PATH`, `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_ALL`, `XDG_RUNTIME_DIR`, and only explicitly registered additional environment names. Additional names containing `TOKEN`, `SECRET`, `PASSWORD`, `CREDENTIAL`, `COOKIE`, or `SESSION_MANAGER` are rejected. No checkpoint path or client-supplied environment is passed. Adapter standard error is bounded to 64 KiB, sanitized as untrusted external text, and never allowed to contain payload bytes by contract.

## Application operations

### Configuration operations

The service provides profile and task list/show/create/update/rename/delete plus configuration validate/reload. All mutations use configuration ETag preconditions and deletion guards for referenced sources. A profile/task name change also requires a runtime-revision precondition because rename propagation mutates runtime labels. Delete also requires a runtime-revision precondition because its validity depends on live references.

Configuration reload compares the accepted and candidate identity sets before adoption. It may accept non-identity field edits, additions, and removals of unreferenced sources. It rejects any candidate that removes or changes the identity of a profile or task referenced by a running or paused session; an external remove-plus-add is never inferred to be a rename and never triggers rename propagation. Reload uses both the expected configuration ETag and expected runtime revision, validates deletion guards and name collisions under the joint commit lock, and adopts either the complete candidate or no change.

### Session operations

The generic service provides session list/show/start/switch/pause/resume/stop and operation/status inspection. It does not provide workspace activation, window focus/move/close, layout, launcher execution, process attachment, signal, or platform-specific recovery methods.

Start and switch are core-only and never invoke an adapter. Start snapshots the source root plus the profile desktop-entry list or task desktop-entry list and publishes the new session UUID for downstream workspace and launcher policy. Pause requires the caller to select one registered adapter ID. Resume and adapter-required paused stop always use the adapter ID retained in the envelope. Running stop is core-only. Recovery reconciles core persistence, status for pending lifecycle intents, and release for terminal release obligations only.

Lifecycle transition mapping is exact:

| Request | Required adapter result | Core transition |
| --- | --- | --- |
| Start | none | create running and active |
| Switch | none | target remains running and becomes active |
| Pause | capture `completed` with valid payload | running to paused |
| Pause | accepted capture `completed` with invalid proposal/capability relation | remain running with `protocol.invalid_message` attention and release obligation |
| Pause | unsupported/rejected/failed/cancelled before acceptance | remain running |
| Pause | known pre-handoff failure, or durable exact-child exit proof plus absent status | remain running; final failed operation, no release obligation |
| Pause | complete handoff then timeout/loss without known acceptance | remain running; acceptance-unknown, recovery required |
| Pause | accepted then unsupported/rejected/failed/cancelled | remain running; terminal operation and release obligation |
| Pause | accepted then unknown | remain running, recovery required |
| Resume | restore `completed` | paused to running and active |
| Resume | restore `completed_with_limitations` | paused to running and active with attention |
| Resume | unsupported/rejected/failed/cancelled before acceptance | remain paused |
| Resume | known pre-handoff failure, or durable exact-child exit proof plus absent status | remain paused; final failed operation, no release obligation |
| Resume | complete handoff then timeout/loss without known acceptance | remain paused; acceptance-unknown, recovery required |
| Resume | accepted then unsupported/rejected/failed/cancelled | remain paused; terminal operation and release obligation |
| Resume | accepted then unknown | remain paused, recovery required |
| Stop running | none | remove session |
| Stop paused, adapter discard not required | none | remove checkpoint and session atomically |
| Stop paused, adapter discard required | discard `completed` | remove checkpoint and session |
| Stop paused, required discard | unsupported/rejected/failed/cancelled before acceptance | remain paused; no release obligation |
| Stop paused, required discard | known pre-handoff failure, or durable exact-child exit proof plus absent status | remain paused; final failed operation, no release obligation |
| Stop paused, required discard | complete handoff then timeout/loss without known acceptance | remain paused; acceptance-unknown, recovery required |
| Stop paused, required discard | accepted then unsupported/rejected/failed/cancelled | remain paused with attention and release obligation |
| Stop paused, required discard | accepted then unknown | remain paused, recovery required |

Successful resume deletes its checkpoint only in the same transaction that commits running. Capture or restore claims of exactness are presentation qualifiers and never weaken these transition requirements.

### Public projections

The version 1 public projection is a complete revisioned object containing:

- configuration ETag and ordered profiles with name, root, desktop-entry IDs, and ordered tasks with name and desktop-entry IDs;
- runtime revision, active session ID, and ordered live sessions with UUID, generated name, source IDs/labels, number, lifecycle, logical workspace-group ID equal to session UUID, immutable source snapshot, and valid lifecycle actions;
- for paused sessions, checkpoint ID plus the safe envelope fields `adapter_id`, `format_id`, `format_version`, `created_at_unix_ms`, payload length, `preservation`, `activity`, `restore_expectation`, and `discard_requires_adapter`; the integrity digest remains internal;
- adapter-registry source ETag, adapter-generation UUID, and the accepted registered-adapter projection with ID, optional deterministic display name from hello, protocol version, validation availability, independent capabilities, supported format ranges, and bounded last-unavailable reason from the last startup or explicit reload;
- operations using the exact kinds, monotonic phases, conditional outcome/error, historical target UUID, timestamps, and revision fields defined in [Public operation records](#public-operation-records);
- attention conditions with stable code, severity, affected core IDs, first-seen revision, and acknowledgement.

Subscriptions publish the same complete configuration, adapter-registry, and runtime projections after a snapshot-first handshake. Clients derive their own presentation and automation models.

No public object contains raw compositor identifiers, windows, geometry, layouts, focus, monitor state, PIDs, process-group identifiers, shell component state, key combinations, or decoded payload fields.

## CLI and IPC

The daemon runs in the foreground and owns one owner-only versioned Unix socket. Messages are bounded newline-delimited UTF-8 JSON with request UUIDs, structured errors, operation UUIDs, runtime revisions, configuration and adapter-registry source ETags, adapter generations, and snapshot-first subscriptions.

The version 1 method inventory is closed and contains exactly:

```text
system.hello              system.status
config.get                config.validate              config.reload
profile.list              profile.get                  profile.create
profile.update            profile.delete
task.list                 task.get                     task.create
task.update               task.delete
session.list              session.get                  session.start
session.switch            session.pause                session.resume
session.stop
adapter.list              adapter.get                  adapter.compatibility
adapter.reload
operation.get
attention.list            attention.acknowledge
event.subscribe
```

`session.start` accepts exactly one profile name and optional task name plus runtime/configuration preconditions; it accepts no adapter or platform target. `session.pause` requires a session ID and adapter ID. `session.resume` and `session.stop` accept only the session ID and preconditions and use persisted checkpoint metadata where needed. `adapter.compatibility` accepts an adapter ID plus checkpoint ID and returns metadata only; it never returns payload bytes.

Methods use strict parameter objects and stable versioned enums. Unknown methods, fields, and safety-relevant enum values fail without mutation.

### Public IPC protocol version 1

Each connection starts with `system.hello`; any other first method fails with `protocol.handshake_required`. The client offers exactly one protocol version in version 1. A server accepting version `1` returns the negotiated version, server version, runtime revision, configuration ETag, adapter-registry source ETag, adapter generation, and maximum line size. An unsupported version closes the connection after one error response. Compatible additions require a new negotiated protocol version; version 1 never silently accepts unknown fields, methods, enum values, or event kinds.

One request, response, snapshot, event, or subscription-error object occupies one UTF-8 JSON line of at most 16,777,216 bytes (16 MiB), including its terminating newline. The bound applies to the complete combined message and all its projections plus framing. Requests have exactly `protocol_version`, `request_id`, `method`, and `params`. Responses repeat `protocol_version` and `request_id`, have `type: "response"`, and contain exactly one of `result` or `error`. `request_id` is a client-generated UUID unique for the connection. Duplicate in-flight or reused IDs are rejected. A mutation result includes its durable `operation_id` when it creates a public operation and the committed `runtime_revision`, `configuration_etag`, `adapter_registry_source_etag`, and/or `adapter_generation` that it changed.

All public size and capacity checks use the same canonical compact JSON encoder as IPC publication. It emits no insignificant whitespace; sorts the member names of every object by their UTF-8 bytes recursively; preserves array order; writes non-ASCII scalar values directly as UTF-8; escapes `"` and `\\`; uses the two-byte escapes `\b`, `\t`, `\n`, `\f`, and `\r` where applicable; encodes every other U+0000–U+001F control as lowercase `\u00xx`; and does not escape `/` or otherwise optionally escape Unicode. Integer spelling is minimal decimal with no leading zero. The one terminating LF is part of the encoded size. The encoder is a shared persistence/application component: alternate serializers may parse messages but cannot decide admission capacity or produce public response, snapshot, event, or subscription-error bytes. Incoming requests need not use canonical member order or compact spacing, but their decoded values and complete framed size remain subject to the same protocol bounds.

Public UUIDs, machine IDs, names, roots, desktop-entry IDs, files, collections, and projections use their owning domain/storage bounds. Additionally, `client_name` is 1–128 UTF-8 bytes without control characters; public stable codes and field paths are at most 128 ASCII bytes; human messages are at most 1,024 UTF-8 bytes; event kinds are closed enums; and opaque cursors are at most 512 ASCII bytes. No implementation-defined smaller limit is permitted for a valid version 1 message.

Errors contain `code`, `message`, optional `field`, optional `current_runtime_revision`, optional `current_configuration_etag`, optional `current_adapter_registry_source_etag`, optional `current_adapter_generation`, and optional `operation_id`. `message` is non-normative. The stable version 1 code families and CLI exit mappings are:

| Error family | CLI exit |
| --- | ---: |
| success | 0 |
| `validation.*`, `protocol.*` | 2 |
| `not_found.*` | 3 |
| `conflict.*`, `lifecycle.*` | 4 |
| `adapter.unavailable`, `adapter.incompatible`, `adapter.unsupported` | 5 |
| `checkpoint.*`, `persistence.*`, `recovery.*` | 6 |
| `timeout.pre_acceptance` | 7 |
| `outcome.acceptance_unknown`, `outcome.indeterminate` | 8 |
| `internal.*` | 70 |

CLI transport or daemon-unavailable failures use exit `69`. Human output and messages are not stable interfaces. `--json` prints exactly the response result or error object and never wraps it in human text.

`timeout.pre_acceptance` is returned only when non-acceptance is safe: request handoff did not complete, or durable exact-child exit proof exists and status is absent. A mere lack of an observed acceptance envelope after complete handoff uses `outcome.acceptance_unknown`, never `timeout.pre_acceptance`.

All list methods return `{ "items": [...], "next_cursor": null|string }`. They accept optional `limit` from 1 through 200, default 100, and optional opaque `cursor` of at most 512 bytes. Order is stable for one revision; a cursor embeds its source revision and fails with `conflict.cursor_stale` after that collection changes. `profile.list` and `task.list` use configuration ETags; adapter lists use the adapter generation; session and attention lists use runtime revisions. Complete snapshots are not paginated and always fit the 16 MiB line bound because their source documents and collections are bounded above.

Method parameters and successful results are normative:

| Method group | Required/optional parameters | Result |
| --- | --- | --- |
| `system.hello` | `client_name`, `protocol_version` | negotiated protocol and server metadata |
| `system.status` | none | complete health, configuration ETag, adapter-registry source ETag, adapter generation, runtime revision, recovery gate |
| `config.get` | none | complete configuration projection and ETag |
| `config.validate` | exactly one of `path` or `document` | validation report; no mutation |
| `config.reload` | expected configuration ETag and runtime revision | complete adopted configuration, new ETag, and unchanged current runtime revision; reload validates runtime guards but never rewrites runtime data |
| `profile.list/get` | list paging; or profile name | profile projection(s) and ETag |
| `profile.create/update/delete` | profile data or patch, expected ETag; update/delete include profile name; name-changing update and delete also require expected runtime revision | resulting configuration projection and ETag, plus runtime revision when required |
| `task.list/get` | profile name plus paging; or profile and task names | task projection(s) and ETag |
| `task.create/update/delete` | profile name, task data or patch, expected ETag; update/delete include task name; name-changing update and delete also require expected runtime revision | resulting configuration projection and ETag, plus runtime revision when required |
| `session.list/get` | list paging; or session UUID | session projection(s) and runtime revision |
| `session.start` | profile name, optional task name, expected ETag and runtime revision | operation and created session projections |
| `session.switch/pause/resume/stop` | session UUID and expected runtime revision; pause also requires adapter ID | operation and resulting runtime projection |
| `adapter.list/get` | list paging; or adapter ID | safe adapter projection(s), adapter-registry source ETag, and adapter generation |
| `adapter.compatibility` | adapter ID, checkpoint ID | compatibility metadata only |
| `adapter.reload` | expected adapter-registry source ETag and adapter generation | complete accepted adapter projection, source ETag, and resulting adapter generation |
| `operation.get` | operation UUID | public operation projection |
| `attention.list` | paging and optional acknowledged filter | attention projections and runtime revision |
| `attention.acknowledge` | attention ID and expected runtime revision | updated attention and runtime revision |
| `event.subscribe` | none | subscription acknowledgement followed by snapshot/event stream |

Create objects contain all required fields and no server-owned fields. Update objects contain `changes`, with at least one mutable field; rename uses the `name` change and therefore invokes rename propagation. Null is accepted only where a field is explicitly optional. A delete method call is itself explicit confirmation and contains no separate confirmation, cascade, or force flag. Every mutation requires all relevant preconditions shown above; absent or stale preconditions fail before mutation.

`event.subscribe` always starts from a complete snapshot and version 1 has no last-seen tuple or event-resume parameters. The subscribe request line must be the only client data buffered for that connection; already buffered trailing bytes reject the request with `protocol.invalid_request` before subscription begins. Its ordinary response result is exactly `{ "subscribed": true }`. After that response, the connection becomes server-stream-only: the server emits the snapshot and later events, and any subsequent byte received from the client causes immediate connection closure without a response. Commands and additional subscriptions require separate connections. The initial unsolicited snapshot has no `request_id`, `event_id`, or `kinds` field:

```json
{
  "adapter_generation": "UUID",
  "adapter_registry_source_etag": "sha256-hex",
  "configuration_etag": "sha256-hex",
  "projections": {
    "adapter_registry": {},
    "configuration": {},
    "runtime": {}
  },
  "protocol_version": 1,
  "runtime_revision": 41,
  "type": "snapshot"
}
```

All three snapshot projections are non-null complete replacements corresponding to exactly the advertised runtime revision, configuration ETag, adapter-registry source ETag, and adapter generation. A later event has exactly this logical shape:

```json
{
  "adapter_generation": "UUID",
  "adapter_registry_source_etag": "sha256-hex",
  "configuration_etag": "sha256-hex",
  "event_id": "UUID",
  "kinds": ["configuration.changed", "runtime.changed"],
  "projections": {
    "adapter_registry": null,
    "configuration": {},
    "runtime": {}
  },
  "protocol_version": 1,
  "runtime_revision": 42,
  "type": "event"
}
```

`kinds` is non-empty, contains no duplicates, and follows the fixed order `configuration.changed`, `adapter_registry.changed`, `runtime.changed`, `operation.changed`, `attention.changed`, `server.shutting_down`. `configuration` is non-null exactly when `configuration.changed` is present. `adapter_registry` is non-null exactly when `adapter_registry.changed` is present; that kind requires a changed adapter generation, although the source ETag may remain unchanged when explicit reload changes validation availability. Any change to runtime, including its operation or attention subcollections, requires `runtime.changed` and a non-null complete `runtime` replacement. `operation.changed` and `attention.changed` are additional cause qualifiers and therefore may appear only together with `runtime.changed`; they never replace it. `server.shutting_down` requires no replacement projection. Fields not selected by their kinds are present as JSON null, not absent. A configuration/runtime rename transaction is one event containing both non-null projections; no subscriber can observe half of the commit. Per-connection order is increasing by committed publication order.

The 16 MiB line limit applies to the complete combined snapshot or event, not to each projection separately. Before committing any mutation, capacity validation encodes both the resulting combined snapshot and the corresponding maximum public event with the canonical encoder and rejects a state that would exceed either bound with `recovery.capacity_exceeded`. Maximum-bound fixtures must prove that every otherwise valid source state remains representable.

Subscription establishment is atomic with publication. Under the same publication lock used to order committed events, the daemon registers the connection queue, captures one coherent runtime-revision/configuration-ETag/adapter-registry source-ETag/adapter-generation tuple, and enqueues that tuple's complete snapshot as the first item before releasing the lock. A commit before registration is represented in the snapshot; a commit after registration is queued after it. No mutation can fall between snapshot capture and queue registration.

From the moment the initial snapshot is enqueued, each subscription queue holds at most 64 messages and at most 33,554,432 encoded bytes, including the queued snapshot but excluding the item currently being written. Every subscription acknowledgement, snapshot, event, and terminal subscription-error write uses the injected monotonic clock and closes the connection if it makes no forward byte progress for 5 seconds or does not finish within 30 seconds total; successful partial progress restarts only the stall interval, never the total deadline. Enqueuing an event that would exceed either queue limit atomically discards queued events and requests closure. If no line has been partially written, the writer makes one nonblocking attempt to write the exact canonical line `{"code":"subscription.resync_required","protocol_version":1,"type":"subscription_error"}\n` from capacity reserved outside the queue limits, then closes whether the attempt is complete, partial, or blocked. If a line is already partial, the writer closes immediately without appending another JSON object to the incomplete frame. EOF, a partial frame, the terminal error, or a write-stall/total-deadline closure all require the client to discard prior state, reconnect on a new connection, and accept a new complete snapshot; version 1 never resumes from a retained tuple.

CLI commands mirror the generic API. Human output is not a parsing contract; `--json` is versioned. A client timeout does not cancel or signal a handed-off lifecycle operation, whether acceptance is unknown or known. Clients never read runtime or checkpoint files directly.

## Errors, attention, and recovery

Stable error families include validation, not found, conflict, lifecycle, adapter unavailable/incompatible/unsupported, checkpoint invalid, persistence, timeout, acceptance-unknown/indeterminate outcome, and internal failure.

Attention records are durable and separate from logs. Acknowledgement changes presentation only; it does not alter lifecycle, adapter state, checkpoint compatibility, or recovery authority. Version 1 exposes no force, assume-failed, forget, replay, or manual recovery-resolution operation. Administrators may repair or compatibly upgrade an adapter outside the core; startup and steady-state reconciliation then retry only safe status and release controls automatically. For each unresolved operation, retry is immediate when recovery starts, then waits 1, 2, 4, 8 seconds and so on to a maximum 300-second interval, with at most one in-flight control request per operation. Only durable state-machine progress resets the next interval to one second: a phase advance, newly committed exact-child exit proof, newly authoritative terminal result, or completed release. A successful response that repeats already known accepted/non-terminal state, an unchanged `not_found.operation`, a transport error, or an attention acknowledgement is not progress and continues the capped sequence. Tests use the injected monotonic clock. `operation.get` remains read-only.

On startup the daemon:

1. acquires the state lock;
2. validates and resolves retained filesystem transactions;
3. validates configuration and core runtime revisions and the exact combined snapshot bound; an externally assembled over-bound combination enters the read-only recovery gate rather than starting with an unpublishable state;
4. validates every referenced envelope, payload length, and digest without parsing payload contents;
5. loads only explicitly registered subprocess adapters and validates their hello metadata;
6. checks compatibility for checkpoints and pending operations;
7. queries status only where the generic contract permits;
8. resolves terminal accepted adapter operations and atomically creates any missing release obligation for them; it resolves absent acceptance-unknown operations only when durable exact-child exit proof exists, otherwise retaining them unless status proves acceptance or a terminal result;
9. retries pending idempotent adapter releases and durably removes only completed obligations;
10. applies terminal public-history retention without removing recovery evidence;
11. publishes one coherent recovered core snapshot before accepting mutations.

If neither current nor previous core state is valid, the daemon remains read-only and does not assume an empty state. Missing or incompatible adapters do not corrupt checkpoints; affected operations remain unavailable with stable diagnostics.

## Privacy and security boundary

Session Manager is a same-user organization tool, not a security boundary. Owner-only files and sockets prevent accidental cross-user access but do not protect against other code running as the same user.

Logs exclude payload bytes, desktop-entry contents, raw client messages, environment values, credentials, and adapter-private diagnostic blobs. Payload inspection requires an explicit adapter-owned diagnostic path outside ordinary core APIs.

## Testing strategy

### Test adapter roles

The test roles are distinct:

- the **application fake port** is an in-process implementation of the typed adapter port used by application and lifecycle unit tests; it starts no subprocess and tests no wire framing;
- the **fake subprocess adapter** is a controlled test executable used for transport, framing, descriptor, timeout, crash, and malformed-protocol cases; it may import test-only fixture support but is never production discovery;
- the **standalone sample adapter** is independently built outside the production Cargo package using only the published wire contract; it proves third-party implementability, durable accepted/terminal records, compatible upgrade, status, capture re-export, and release.

No one role substitutes for evidence assigned to another.

### Default core suite

The default suite uses temporary XDG roots and fake implementations for storage, clock, UUID generation, and the adapter port. It cannot search executable paths for adapters, connect to graphical sockets, inspect a display, use a user bus, or signal non-fixture processes.

Coverage includes:

- profile/task validation and format-preserving edits;
- naming, numbering, rename, deletion guards, and lifecycle properties;
- checkpoint envelope validation and byte-exact opaque payload round-trips;
- atomic storage and fault injection at every transaction boundary;
- durable lifecycle-intent to conditional release-obligation handoff, release retry, monotonic terminal operation-history retention across clock changes, and capacity gates;
- adapter capability-qualified formats, registry reload, compatibility, timeout, cancellation, failure, status/release, capture re-export, acceptance-unknown, and indeterminate-result models;
- IPC framing and exact bounds, concurrency, pagination, maximum projections, subscriptions, and restart recovery;
- privacy and path-containment traps.

### Adapter conformance suite

The repository defines reusable contract fixtures for any adapter implementation. The fake subprocess adapter proves generic wire paths; the application fake port proves typed application behavior without wire claims; and the standalone sample adapter proves independent implementability. Concrete adapters run the same applicable conformance cases in their owning project, including exact payload handoff and capture re-export, version mismatch, capability-qualified format gating, pre-handoff, acceptance-unknown, pre-acceptance-final, and accepted sequences, operation identity, retained restart and compatible-upgrade status, idempotent release, bounded failure, and no secret leakage.

### Live and isolated integration

Concrete desktop tests are not part of the core MVP. An adapter project may use a disposable VM or isolated login. Live mutation requires explicit approval and cannot substitute for default or conformance tests.

## Technical acceptance

The core technical design is accepted for implementation after the design validation trace confirms these schemas and enums. The implementation must then prove:

1. no core dependency or public type names a concrete desktop technology;
2. profiles, tasks, sessions, and persistence work with the application fake port and no graphical environment;
3. checkpoint payload bytes round-trip unchanged across storage, restart, and adapter handoff;
4. incompatible adapters and corrupt payloads fail before handoff;
5. interrupted non-idempotent adapter operations are not replayed automatically;
6. terminal core commits for accepted adapter operations preserve a durable release obligation until idempotent adapter release completes, while core-only and pre-acceptance outcomes create none;
7. completed capture re-export is byte-identical and incomplete recovery data is never authoritative;
8. arbitrary clients can consume every maximum valid generic snapshot within the public protocol bound;
9. default tests cannot reach an installed adapter or live desktop.
