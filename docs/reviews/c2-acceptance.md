# C2 acceptance: current application lists

- Status: **accepted**
- Evaluated: 2026-11-05
- Source: working tree based on `d7318685cda07b34f56af7f50aadc6ce0bf17d59`, retaining the pre-existing C0/C1 work.
- Contract: [C2 gate](../implementation-plan.md#c2--current-application-list), [application resolution](../technical-specification.md#application-resolution) and [ADR 0012](../decisions/0012-discover-project-profiles-and-resolve-xdg-applications.md).

## Delivered boundary

`applications::ApplicationEnvironment::from_values` validates explicitly supplied startup values and resolves XDG search locations without reading the ambient environment. `ApplicationResolver` reads current files for each query. `Core::applications` selects the active recorded project name and obtains request-scoped filesystem context, never a persisted root/app snapshot. `desktop_entry.rs` owns text parsing; `safe_fs.rs` supplies bounded no-follow directory access. A replaceable metadata-only executable lookup allows tests to trap `TryExec` checks without touching real programs.

The resolver returns launcher-independent typed application results. It does not execute Exec/TryExec, read icon contents, launch a GUI, discover adapters, watch files, cache application lists, or write runtime/project configuration. The authoritative technical specification records parsing, precedence, bounds and race/error details; this review records evidence rather than duplicating that contract.

C3 must capture the real process environment once at daemon startup and retain a resolver for requests. The socket, wire handlers, CLI, installation and launcher-client example are **not** implemented or accepted by C2. Tasks and pause/resume remain deferred.

## Acceptance trace

`src/c2_tests.rs` contains **36 new fixtures**, alongside the retained 43 C1/helper tests.

| Condition | Representative evidence |
| --- | --- |
| Explicit XDG defaults, validated roots, priority and lexical deduplication | `injected_xdg_defaults_priority_and_validation_do_not_touch_the_host`; defaults are inspected as data only, never enumerated against the host |
| Recursive desktop IDs, within-root collisions, root priority and deterministic output | `installed_priority_recursive_ids_ties_and_output_order_are_deterministic`; repeated canonical results compare byte-for-byte |
| Masking before visibility, malformed-winner errors, ignored masked files | `hidden_and_ineligible_high_priority_entries_mask_lower_files`; `malformed_installed_files_fail_with_stable_private_errors` |
| Desktop tokens, locale-independent fields, standard escapes/booleans/groups | Desktop visibility/text/parser fixtures include CRLF, bare-CR rejection, BOM/invalid UTF-8, duplicate keys/groups, unsupported encoding, mutually exclusive visibility lists, localized/vendor/action fields, and unexpanded Exec text |
| Exclusions followed by overlays and reintroduction | `exclusions_then_local_overrides_can_reintroduce_and_remove_ids`; ineligible local entries also mask installed ones |
| Strict local errors and no unsafe traversal | Local symlink/ancestor/directory/FIFO/non-UTF-8 fixtures, exact exclusion syntax/duplicates/control characters, malformed local files, unreadable files and ignored unrelated regular files |
| Installed no-follow discovery with truthful I/O errors | Symlink/non-regular-entry, disappearing/appearing root, non-directory root and unreadable-root/file fixtures |
| Missing/no-active fallback and session independence | No-active, rename/removal/reused-name, missing source/project and detached-context fixtures; session identity is unchanged |
| Context changes without restart or runtime writes | `queries_reflect_installed_and_project_edits_without_state_writes_or_restart` compares runtime bytes; switching/restart and concurrent-switch fixtures prove one active-name selection without holding the runtime lock across filesystem reads |
| Mid-request replacement does not serve detached overrides | `project_replacement_after_read_discards_old_overrides_and_next_query_uses_new_files` |
| TryExec is a current metadata/access probe, never execution | Injected PATH, absolute paths, executable symlinks/removal/permission changes, a trap/spying lookup, and an executable marker script whose marker is never created |
| Files, fields, traversal, unique/local/final counts and complete reply bounds | Boundary fixtures include ignored parent siblings in the traversal budget, sparse oversized files, 4096/4097 IDs, local-file and final-result overflow, and the exact 16 MiB response versus one extra byte. Capacity accounting is checked against the actual canonical response envelope, including LF. No truncation is accepted. |
| No ambient host/application/adapter/process discovery | `c2_production_sources_never_read_ambient_environment_or_execute_processes`, explicit temporary roots in every resolver fixture, and the complete sandbox run below |

The strict file-error policy is intentional and owner-approved at C0; this evidence is not a claim of accepting every vendor desktop-entry extension. No installed desktop-entry inventory was used to validate compatibility. The parser is a discovery parser, not a command-line launcher or full semantic linter for every extension.

## Commands and results

Pinned toolchain: Rust/Cargo **1.96.1**.

```sh
RUSTUP_TOOLCHAIN=1.96.1 CARGO_NET_OFFLINE=true make check
RUSTUP_TOOLCHAIN=1.96.1 make test-isolated
git diff --check
```

All passed: formatting, warnings-as-errors Clippy, all-target build, warnings-as-errors rustdoc, **79 tests normally and 79 sandboxed**, and whitespace checks. Permission-denial assertions are conditional for UID 0, which bypasses ordinary Unix mode checks.

The existing `tools/test-isolated.sh` runs only current Cargo test artifacts with a read-only host/repository view, hidden host home/user-runtime/application roots, synthetic HOME/XDG values, a cleared environment, temporary writable storage, and isolated network/PID namespaces. The C2 fixtures inject only temporary application/PATH roots and fixture absolute executable names. A default XDG configuration is constructed for a value-level test but never scanned. No real installed application, user project, live desktop, adapter, or launcher was accessed or modified for acceptance.

## Next stage

C3 provides the owner-only version 2 daemon/API/CLI over the accepted C1/C2 library behavior, startup environment/bootstrap wiring, full protocol/integration fixtures and release documentation. This record does not accept any of that work or alter historical M0–M2/C1 evidence.
