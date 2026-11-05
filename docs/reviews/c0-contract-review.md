# C0 contract review and reuse audit

- Reviewed at: 2026-11-04
- Source: working tree based on `d7318685cda07b34f56af7f50aadc6ce0bf17d59`, including pre-existing uncommitted M2 and replacement-document changes.
- Status: **C0 accepted by the owner, with legacy compatibility explicitly removed**. No implementation files changed by this review.

## Owner decision

The owner approved the project/template, application-resolution and minimal protocol policies, and amended the legacy policy: existing functionality, sessions and checkpoints do not need to remain usable. The [accepted technical contract](../technical-specification.md#initial-release-technical-contract-version-2) and [ADR 0013](../decisions/0013-isolate-version-two-state-and-gate-legacy-recovery.md) reflect this clean break.

No legacy migration, detection, startup gate, recovery, preservation opt-in or rollback compatibility is required. Separate v2 paths remain; obsolete files are ignored rather than automatically deleted. No task format or adapter capture schema is selected; L1/L2 remain separately gated. C1 may begin.

## Gate coverage

| C0 condition | Contract location/evidence | Review outcome |
| --- | --- | --- |
| Source setting, names, discovery and start lookup | Technical specification: Settings and project lookup | Accepted |
| Template selection, suffixes, safe copy, races, recovery | Templates and project creation | Accepted |
| Exclusions, desktop identity, XDG precedence/visibility, errors/bounds/fallback | Application resolution | Accepted |
| Versioned methods/results, CLI, errors, necessary revisions/events | Public API and CLI version 2 | Accepted |
| Minimal runtime, legacy state and rollback | Runtime and compatibility; ADR 0013 | Accepted |
| Distinguish legacy CRUD, snapshots and capture inputs | Historical v1 boundary; reuse audit below | Audited; implementation unchanged |
| Every C0 acceptance condition reviewed before code | This table | Accepted by owner before implementation |

## M1/M2 reuse audit

Static inspection of `src/` and its inline test modules, not a new acceptance test run. Existing tests describe their original contract; none proves C1–C3 as a whole.

| Area | Keep/adapt | Remove, replace or defer |
| --- | --- | --- |
| `src/domain.rs` | `allocate`/`generated_name`: highest-live + 1, exhaustion, global-name collision and stopped-number reuse remain valid. Port `allocation_obeys_highest_live_and_global_names`, `allocation_reuses_stopped_numbers_but_not_lower_live_gaps_or_maximum`, and `fds_and_qmk_sequences_are_per_source_and_stopped_numbers_are_reusable` to new session fixtures. | Replace `Configuration`, `Profile`, `Task`, `LaunchSource` and `Session` for v2. Remove root/app snapshots, rename propagation, deletion guards, live-source reload policy. Old subset/bounds, rename, deletion and reload tests are historical, not v2 acceptance. New filesystem names require new validators, not the old logical-name validator. |
| `src/configuration.rs` | Strict UTF-8/version/unknown-field/bounded-read checks and synchronized replacement are useful patterns. Identity/symlink-replacement tests remain useful safety examples. | Replace schema-1 profile/task parsing and lossless CRUD; v2 has no config mutation/reload API. Existing `zero_profiles_*`, CRUD and ETag tests do not validate settings v2. New tests must cover v2 paths/settings, no-follow roots and ignored legacy artifacts. |
| `src/runtime.rs` | Keep UUID identity, consistent active ID, bounded decoding, revision exhaustion and durable replacement invariants. Port `runtime_active_session_is_consistent_with_sessions`, strict JSON, sparse-file and size-limit cases with the new schema. | Replace `RuntimeState` and projection limits. No shutdown/time/history/attention/operation/release/adapter state in v2. `runtime_store_selects_the_highest_valid_revision_without_rewriting_corruption` is **not** the accepted v2 recovery policy: invalid current requires a gate, not silent fallback. New fixtures must prove that a completed stop cannot be silently undone. |
| `src/transaction.rs` | Failure injection, synchronized candidates, exact digest validation, complete-set rather than hybrid recovery are evidence for unchanged storage principles. `manifest_rejects_path_traversal_and_never_carries_bytes`, manifest roundtrip and interruption fixtures remain v1 safety evidence. | Do not wire the old configuration/runtime/checkpoint coordinator or manifest recovery into v2 startup. V2 ignores legacy manifests; no detection/recovery gate is needed. Add v2-specific atomic runtime and project-publication failure tests; old multi-file success is not their acceptance. |
| `src/checkpoint.rs`, `src/adapter_contract.rs` | Opaque-byte, digest/length, safe-identifier and privacy guarantees remain useful historical/deferred L2 evidence; retaining old code/tests or checkpoint usability is not required. | Do not open, migrate, quarantine, clean up, or invoke adapters for these stores from v2. Runtime backup/checkpoint reference assumptions must be reviewed again for L2. |
| `src/canonical_json.rs` | Deterministic recursive key ordering and escaping test still proves the encoding primitive. Reuse with new response-bound fixtures. | Old projection capacity does not prove new app/project/session result capacity. |
| `src/test_support.rs`, `src/storage_port.rs`, `src/application.rs` | Reuse deterministic UUID/clock ports, sentinel-owned temporary roots and forbidden-endpoint traps. Existing isolation tests prove only those helpers' behavior. | Add injected XDG/PATH/desktop environment roots and enforce traps through real new entry points. Trap objects that are never wired do not prove absence of host reads. |
| `src/interface.rs`, `src/main.rs` | Preserve one-package structure. | API version marker and M5 stub are not a v2 implementation. Replace old inventory/shape assumptions; add protocol/CLI and unsupported-v1 tests in C3. |

## Evidence and next step

Read the functional/technical specifications, implementation gate, ADRs 0002/0003/0010/0012 and relevant implementation/test definitions. Enumerated inline test cases using `rg`; reviewed existing worktree status before edits. Historical M0–M2 acceptance records were not edited. No live desktop, adapters, host XDG enumeration, migration, or implementation test was run.

Owner approval recorded; ADR 0013 and the contract are accepted, affected ADR notices updated, and C0 checklist complete. Next: implement C1 with new isolated fixtures; do not transfer old acceptance checkmarks. The original preservation/gating proposal was rejected, not implemented.
