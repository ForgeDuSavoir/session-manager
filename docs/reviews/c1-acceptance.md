# C1 acceptance: projects, templates and running sessions

- Status: **accepted**
- Evaluated: 2026-11-05
- Source: working tree based on `d7318685cda07b34f56af7f50aadc6ce0bf17d59`, including the pre-existing uncommitted C0/M2 work.
- Contract: [C1 gate](../implementation-plan.md#c1--projects-templates-and-independent-sessions), [initial-release technical contract](../technical-specification.md#initial-release-technical-contract-version-2), ADRs [0012](../decisions/0012-discover-project-profiles-and-resolve-xdg-applications.md) and [0013](../decisions/0013-isolate-version-two-state-and-gate-legacy-recovery.md).

## Delivered boundary

`application::Core` is the serialized library entry point. It loads strict v2 settings from explicitly supplied `configuration::Locations`, owns the single-writer lock, and exposes project/template discovery, creation, running-session inspection/start/switch/stop, and recovery diagnostics. The supplied XDG parent directories and config directory must exist; the service creates private state/runtime leaf directories. The library does not inspect the ambient environment.

`projects.rs` and `safe_fs.rs` implement descriptor-relative no-follow traversal and exclusive/no-replace publication. `domain.rs` and `runtime.rs` implement name-only sessions and synchronized v2 persistence. The obsolete profile CRUD, rename/delete guards, frozen snapshots and operation/adapter runtime were replaced; old checkpoint/transaction/adapter source modules and their unused direct dependencies were removed. Useful canonical encoding and isolated fixture/UUID primitives remain. No v1 files are inspected or modified by the new core.

Application resolution is **not** implemented here (C2). The executable remains a foundation stub; environment/bootstrap wiring, owner-only socket, wire handlers and CLI belong to C3. Neither tasks nor pause/resume is enabled.

## Acceptance trace

All implementation fixtures are in `src/c1_tests.rs` (38 tests), supplemented by five canonical-encoding/test-helper tests.

| Condition | Representative evidence |
| --- | --- |
| Current, immediate, metadata-free projects | `discovery_is_immediate_current_and_metadata_free`; missing/replaced source and strict names/settings fixtures |
| Built-in/custom template selection and first-free numbering | `built_in_and_custom_templates_copy_only_contents_with_private_modes`; `template_selection_reserved_names_and_first_free_all_entry_types` |
| Bounded no-follow copies, safe modes and malformed inputs | Unsafe/hardlink/FIFO/non-UTF-8, symlink ancestor/root, unreadable file, exact/excess depth/byte/entry and inherited-special-permission fixtures |
| No partial projects, no overwrite, concurrency | `partial_copy_is_invisible_to_concurrent_discovery`; interrupted copy, concurrent create, staging/source substitution fixtures; `external_creation_race_never_overwrites_the_winner` inserts a winner **after** the vacancy check to exercise no-replace rename failure/retry |
| Session identity, independent numbering, activity and restart | `sessions_survive_rename_removal_reuse_and_restart_without_snapshots`; highest-live/gap/reset, concurrent starts, switch no-op, revision/number exhaustion, UUID collision and owner-lock fixtures |
| Global generated-name collision guard | `exhaustion_collision_and_uuid_collision_do_not_mutate_state` tests the defensive allocator guard with a synthetic collision. Project-only names are injective under the final numeric suffix; task/project collisions are deferred to L1, not claimed as C1 evidence. Persisted malformed generated names are rejected by runtime validation. |
| No project link, rename cascade, app/root snapshot or directory requirement for switch/stop | Rename/removal/reuse fixture compares the persisted session field set and exercises UUID-only switch/stop after directory deletion or replacement with a link |
| Durable all-or-nothing runtime and truthful uncertain results | Candidate/backup/replacement fault boundaries, first-commit interruption, abandoned candidate, external-edit/state-root substitution, uncertain stop and corrupt-current fixtures. Current validity wins over a higher previous revision; invalid/missing current with an existing backup never silently resurrects a stopped session. |
| Clean break without legacy startup gates | `legacy_artifacts_are_ignored_unchanged_even_when_corrupt_or_pending` |
| No live desktop, adapter, host application discovery or process effect | `c1_has_no_legacy_adapter_or_host_discovery_paths` checks production sources for forbidden dependencies; full suite also runs under the sandbox below. The old standalone trap test is not treated as sufficient evidence by itself. |

Orphan staging paths are reported by `Core::incomplete_creations()` and retained for explicit inspection, not automatically deleted on restart. A runtime recovery gate is observable through `Core::recovery_required()`; no previous revision is silently adopted. A first-commit interruption after backup publication can require explicit recovery even though no session was admitted. These are conservative recovery states, not claimed successful operations.

## Commands and results

Toolchain: Rust/Cargo **1.96.1**, the pinned project toolchain.

```sh
RUSTUP_TOOLCHAIN=1.96.1 CARGO_NET_OFFLINE=true make check
RUSTUP_TOOLCHAIN=1.96.1 make test-isolated
git diff --check
```

All passed: formatting, warnings-as-errors Clippy, all-target build, warnings-as-errors rustdoc, **43 tests normally and 43 in isolation**, and whitespace checks. The permission-bit fixture is designed to skip its denial assertion for UID 0, which bypasses ordinary file permissions.

`make test-isolated` requires Linux, `bwrap`, Python 3 and permission to create namespaces. `tools/test-isolated.sh` builds offline, selects only the current Cargo test artifacts, then runs them with a read-only host/repository view, hidden home/user-runtime/application roots, an empty environment replaced with synthetic HOME/XDG locations, private temporary storage, and separate network/PID namespaces. No display or user-bus address is inherited. It fails rather than falling back to a live run if isolation is unavailable. The repository-local default Cargo target directory is expected.

No live session, project, adapter, socket or checkpoint was used for validation. No runtime state migration or desktop operation was performed. Historical M0–M2 records remain unchanged and are not reused as C1 acceptance.

## Next stage

C2 implements current XDG application lists against injected roots, using the accepted exclusion/overlay/fallback policy. C3 provides public access and installation guidance. No C2/C3 completion is implied by this record.
