# 0003: Use a versioned JSON Lines Unix socket API

- Status: Accepted
- Date: 2026-07-15

## Context

Session Manager needs one local authority for configuration and lifecycle mutations while supporting command-line clients, arbitrary local integrations, adapter coordination, queries, and continuous state updates.

The target is a local single-user Linux session. Network access, remote clients, and cross-machine interoperability are outside the first-version scope.

## Decision

Expose one versioned IPC API over an owner-only Unix stream socket under `$XDG_RUNTIME_DIR/session-manager/`. Encode requests, responses, and events as bounded newline-delimited UTF-8 JSON objects. Public server output and admission-capacity checks share one canonical compact encoding, including recursively byte-sorted object keys and a size-counted LF, so valid-state representability is deterministic. Use request UUIDs for correlation, operation IDs for admitted lifecycle mutations, runtime revisions, configuration and adapter-registry source ETags, and a separate adapter-generation UUID for optimistic concurrency and projection identity. Subscriptions are unconditional and snapshot-first. Under the publication lock, registration of the subscriber queue, capture of one coherent revision/ETag/generation tuple, and enqueueing of its initial snapshot are atomic; a mutation is therefore represented either in that snapshot or in a later queued event. Version 1 subscriptions accept no resume tuple and make their connection server-stream-only after acknowledgement; other commands use separate connections. One durable commit that changes multiple public projections is emitted as one event carrying every changed replacement projection, so clients never observe half of a configuration/runtime rename transaction. Subscription queues have normative message and byte limits, monotonic write-stall closure, and a separately reserved best-effort resynchronization line emitted only when doing so cannot corrupt a partial frame.

Provide the same generic lifecycle and configuration API through the `session-manager` command-line client. Except for read-only validation of an explicit configuration file, clients never bypass the daemon to access persistent state.

The public protocol carries platform-independent session, checkpoint-envelope, capability, and operation metadata. It does not expose compositor objects, shell-specific presentation models, keyboard actions, or opaque checkpoint payload contents as structured core data.

## Consequences

Unix sockets provide local permissions and peer checks without opening a network service. JSON Lines is inspectable and straightforward for clients and independently implemented adapters to frame.

One typed contract keeps CLI and downstream integrations aligned. Snapshot-first subscriptions make reconnection deterministic. Protocol versioning permits incompatible evolution, at the cost of explicit compatibility handling and bounded parsing requirements.
