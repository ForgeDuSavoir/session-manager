# 0008: Use durable sagas and conservative recovery

- Status: Accepted
- Date: 2026-07-15

## Context

Session operations span atomic core files and non-transactional adapter requests. The daemon may crash after an adapter effect but before persisting or publishing its result. Blindly repeating an adapter action could duplicate work or damage adapter-owned external state.

Warnings must remain visible independently of logs, while diagnostics must not expose private configuration or opaque checkpoint contents.

## Decision

Represent every accepted non-atomic mutation as a durable saga. Assign a core operation UUID and persist a typed core intent before invoking an adapter or permitting its first external effect. Query recovery status by that durable core UUID only while the lifecycle intent remains pending; record only generic recovery facts and any optional opaque adapter token, but never make recovery depend on receiving the token. Verify the adapter-reported postcondition when the contract supports it. The terminal core-state commit for an operation known to have reached acceptance atomically replaces the lifecycle intent with a durable adapter-release obligation, including when the terminal outcome does not change session lifecycle. From then on recovery invokes only idempotent `operation.release`, where an already absent record is successful, and removes the obligation only after release completes in a later durable commit. A pre-acceptance final creates no obligation. Never replay a non-idempotent adapter action during automatic recovery.

Use stable public error families and durable attention records distinct from lifecycle and logs. Emit sanitized structured diagnostics with operation and session UUIDs for correlation and no independent Session Manager log store.

On startup, reconcile core files, checkpoint integrity, pending intents, and adapter-reported operation status. Adapter-reported absence proves non-acceptance only when the spawning daemon durably recorded exact-child exit after reaping; a later daemon may trust that proof, but without it the intent remains acceptance-unknown and recovery-required. The core never inspects adapter-owned external resources or payload semantics. It blocks unsafe mutations instead of guessing, selecting another adapter, repeating an uncertain request, or claiming an unverified restore.

## Consequences

Adapter effects cannot be transactionally atomic with core files, but clients see a completed lifecycle change only after the recorded adapter outcome and persistent core state agree.

Runtime state contains pending lifecycle intents, adapter-release obligations, recovery metadata, optional opaque adapter tokens, bounded public operation history, and attention conditions. Some interrupted operations remain acceptance-unknown or indeterminate until automatic safe reconciliation observes adapter progress, potentially after an administrator repairs the adapter externally.

Privacy-safe logging reduces diagnostic detail. Adapter-specific diagnosis belongs to the adapter and must still respect the checkpoint confidentiality contract.
