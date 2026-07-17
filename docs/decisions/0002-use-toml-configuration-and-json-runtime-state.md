# 0002: Use TOML configuration and JSON runtime state

- Status: Accepted
- Date: 2026-07-15

## Context

Profiles and tasks are user-managed and benefit from a concise editable representation. Running and paused session metadata is machine-managed and must remain inspectable during recovery. Adapter checkpoint contents are also machine-managed, but ADR 0010 makes them opaque to the core.

Persistent files need predictable Linux locations, schema evolution, validation, format-preserving configuration updates, and crash-safe replacement.

## Decision

Store user configuration in one versioned UTF-8 TOML 1.0 document at `$XDG_CONFIG_HOME/session-manager/config.toml`. Store running and paused core state in versioned UTF-8 JSON under `$XDG_STATE_HOME/session-manager/`, with previous validated revisions, pending lifecycle intents, adapter-release obligations, bounded terminal operation history, and durable transaction metadata as required for recovery. Do not persist completed stopped sessions.

Represent an empty profile list by the absence of `[[profiles]]` tables and reject `profiles = []`. Retain accepted TOML as a lossless syntax document. Programmatic mutations edit the smallest applicable range, preserve all unrelated bytes and comments, and reparse and validate the complete candidate before commit.

Treat profile and task rename as an explicit configuration/runtime transaction. Preserve stable session identity, source association, and number while updating source labels and generated names consistently. An explicit reload never infers rename from an externally edited remove-plus-add: it rejects removal or identity change of a source referenced by a live session and accepts only a complete candidate that satisfies current deletion guards under a joint configuration/runtime precondition. Core-only start allocation and session creation are one serialized atomic commit; no durable pre-commit start reservation is stored.

Store each adapter checkpoint as a core-owned versioned envelope plus integrity-protected opaque payload bytes. Runtime state may reference a separately stored payload. Session Manager preserves accepted payload bytes exactly and never parses, normalizes, merges, partially updates, reconstructs, or migrates their adapter-owned contents.

Reject unknown fields for a declared core schema version. Perform persistent writes through synchronized temporary files and atomic replacement in the destination directory. Keep user configuration, core runtime state, and opaque checkpoint payloads distinguishable by ownership and lifecycle.

## Consequences

TOML remains readable and comment-friendly, while JSON keeps core runtime metadata inspectable. Opaque payloads preserve platform-native state without imposing a universal window or layout schema.

The core cannot diagnose, edit, migrate, or restore checkpoint contents by itself. Resume requires a compatible adapter, and payload portability exists only when that adapter guarantees it.

Crash-safe coordination is more complex when an operation changes configuration, runtime metadata, and checkpoint references, but recovery can validate core metadata and payload integrity without interpreting payload semantics.
