# 0013: Use isolated version two state without legacy compatibility

- Status: Accepted
- Date: 2026-11-04

## Context

ADR 0012 replaces the central-profile model and requires a new API/schema. The C0 draft proposed preserving legacy usability and gating startup on legacy transactions. The owner explicitly rejected that requirement: existing functionality, sessions and checkpoints do not need to remain usable. The other C0 policies were approved.

## Decision

Use the separate v2 settings, runtime and socket paths specified in the [technical contract](../technical-specification.md#initial-release-technical-contract-version-2). Serve only protocol 2. Do not implement legacy detection, migration, recovery, compatibility gates, or preservation opt-ins. Ignore old artifacts, including pending or corrupt transactions. Do not automatically delete or rewrite them; keeping them usable is not a requirement. Never invoke old adapters. Remove obsolete implementation and tests where appropriate, retaining useful independent primitives rather than compatibility machinery.

Keep the single-owner state lock. No events, configuration ETags, adapter generations, public operation history or revision preconditions are required for the initial running-only API. Revisions are observations, not concurrency tokens.

This partially supersedes the v1-specific storage and IPC portions of [ADR 0002](0002-use-toml-configuration-and-json-runtime-state.md) and [ADR 0003](0003-use-a-versioned-json-lines-unix-socket-api.md). Future checkpoint safety remains a separate L2 requirement, not a commitment to old checkpoints.

## Consequences

The replacement can be implemented without legacy behavior or a migration layer. There is no supported conversion or session rollback to v1. Separate paths avoid interpreting incompatible files, while leaving obsolete-file removal to explicit owner action. Historical M0–M2 records remain historical. See the [C0 acceptance review](../reviews/c0-contract-review.md).
