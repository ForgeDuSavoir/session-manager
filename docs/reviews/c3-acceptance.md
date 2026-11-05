# C3 acceptance: public access and initial release

- Status: **accepted for the initial release scope**
- Evaluated: 2026-11-05
- Source: working tree based on `d7318685cda07b34f56af7f50aadc6ce0bf17d59`, retaining the pre-existing C0–C2 work.
- Contract: [C3 gate](../implementation-plan.md#c3--public-access-and-initial-release), [version 2 public contract](../technical-specification.md#public-api-and-cli-version-2), ADRs [0003](../decisions/0003-use-a-versioned-json-lines-unix-socket-api.md) and [0013](../decisions/0013-isolate-version-two-state-and-gate-legacy-recovery.md).

## Delivered boundary

The `session-manager` executable is now a foreground daemon and matching client. `bootstrap.rs` isolates environment capture and safe startup; `interface.rs` implements the closed typed protocol and duplicate-key rejection; `transport.rs` supplies owner-only Unix IPC, bounded framing/admission and client validation; `cli.rs` owns command parsing, rendering and daemon signal handling. Session mutation projections/revisions are captured in the core commit lock, not sampled after another request can commit.

The production daemon never spawns applications, launchers or adapters. The optional [generic menu example](../../examples/launcher-client.py) is a separate downstream client that runs only a user-supplied menu command and returns selected data, never evaluating Exec. [Installation/operation/recovery guidance](../usage.md) and the authoritative [startup/transport details](../technical-specification.md#startup-transport-and-cli-operation) document safe use and the intentional clean break.

No version 1 serving, migration, checkpoint handling, task support, pause/resume, subscription or platform integration is included. Stale socket paths are not automatically removed. These are documented boundaries, not hidden initial-release dependencies.

## Acceptance trace

The suite now contains **101 tests**: 94 library tests (including 15 new C3 protocol/transport fixtures) and 7 executable integration fixtures. All prior C1/C2/helper tests remain in the suite.

| C3 condition | Evidence |
| --- | --- |
| Complete owner-only API and corresponding CLI | Closed eight-command dispatch, strict envelopes/parameters and canonical result/error serialization; `api_full_flow_restart_fallback_and_cli_compatible_results` and `binary_full_flow_default_custom_template_json_errors_and_restart` |
| Default and custom template creation, independent running sessions | Executable flow creates both templates' projects, verifies copied/empty contents, starts/lists/switches/stops and restarts against the same v2 runtime |
| Current app list with no-active/missing-project behavior | API and executable flows plus the retained C2 edit/removal/reuse fixtures; executable client ignores its own invalid config/state/data environment and gets daemon-resolved apps |
| Real socket permissions and peer identity | Socket inode/mode checks, client refusal of nonprivate sockets, actual `SO_PEERCRED` checks on both sides; a mismatched expected-UID fixture exercises rejection before request decoding without creating another user or contacting a live process |
| Invalid/concurrent requests cannot partially mutate | Unknown/duplicate fields, malformed UTF-8/JSON/types/UUIDs, unsupported v1, unknown legacy endpoints, incomplete frames, and concurrent starts. Twelve concurrent replies carry their own committed revisions/numbers. |
| Bounded frames, writes, admission and stalled peers | Exact framed-size/overflow and EOF tests, read stall, trickle progress versus total deadline, write backpressure, and 33rd-connection rejection without admission. Retained C2 fixtures test the actual full 16 MiB reply boundary, including LF. |
| Truthful lost acknowledgement and no automatic replay | `lost_mutation_reply_is_uncertain_without_retry_or_rollback` commits through the service, deliberately loses the reply and verifies one session plus `transport.uncertain`, without another request |
| Restart, cleanup and safe failures | Clean shutdown/restart flows, no overwrite of existing regular/symlink socket entries, inode-checked shutdown cleanup, unsafe runtime/state-parent rejection, invalid startup configuration and owner-lock contention |
| Explicit clean-break compatibility | Executable test retains invalid v1 config, runtime/current/previous, pending transaction and checkpoint bytes unchanged while starting and mutating v2 state; no legacy usability is claimed |
| Generic launcher example | Python example is exercised with the built CLI and an explicitly supplied headless fixture menu; it returns the selected application data and leaves Exec unexecuted |
| No live desktop/home/adapters and no network dependency | All runtime/config/state/project/XDG inputs are temporary fixtures; the entire check also runs under the sandbox described below. There is no production adapter registry or application-process invocation path. |

Fixture daemons are owned child handles with fully replaced environments. Only those known child PIDs are signalled and waited for during test cleanup, with private fixture state; no discovered or active user daemon is stopped. Peer identity testing does not assert a live multi-user integration check. Permission-denial assertions inherited from earlier stages are conditional for UID 0, which bypasses ordinary mode checks.

## Commands and results

Pinned toolchain: Rust/Cargo **1.96.1**.

```sh
RUSTUP_TOOLCHAIN=1.96.1 CARGO_NET_OFFLINE=true make check
RUSTUP_TOOLCHAIN=1.96.1 make check-isolated
bash -n tools/test-isolated.sh tools/check-isolated.sh
git diff --check
```

All passed. Both normal and fully isolated runs pass **101 tests**, formatting, warnings-as-errors Clippy, all-target builds and warnings-as-errors rustdoc. The isolated target also passes `cargo build --release --offline --locked` and reports `session-manager 0.1.0 (protocol 2)` from the release binary. The current-artifact-only sandbox target was also exercised while developing C3.

`tools/check-isolated.sh` mounts the installed toolchain and checkout read-only, exposing only a separate ignored `target/c3-isolated` output tree. It clears the environment, provides a private minimal `/dev`, hides host home/runtime/application roots, creates private temporary storage, and isolates PID/network namespaces. Runtime and process fixtures cannot use host desktop sockets/devices. It disables toolchain auto-install and Cargo network access; dependencies, including the two socket-support transitive crates added at C3, are vendored. The private `/dev/null` is necessary for safe subprocess stdio, not an exposure of host devices.

The CI definition now runs the full isolated target after explicitly preparing isolation tooling. This record describes local executed evidence, not a claimed remote CI result. Isolation is fail-closed if namespace creation/tooling is unavailable; no host policy is weakened automatically.

## Release outcome and limitations

C0–C3 are complete for projects, current app lists and running logical sessions. The implementation/build is ready for the initial release scope; no package publication, installation in the user's live session, or native desktop compatibility test was performed. No credentials, browser profiles, real project/session runtime or checkpoint data was used for acceptance.

L1 tasks and L2 adapter-backed pause/resume remain independent future deliveries. Strict desktop-entry validation and explicit stale-socket/runtime recovery remain as documented. Historical M0–M2/C1/C2 evidence is preserved rather than relabelled as C3 acceptance.
