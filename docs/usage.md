# Installation and operation

Session Manager 0.1.0 provides the [version 2 API](technical-specification.md#public-api-and-cli-version-2): projects/templates, independent running sessions, and current application lists. It does not launch applications, change desktops, close windows, or suspend processes. Tasks and pause/resume are not included.

## Build and install

Requirements: Linux with `/proc` mounted, an installed Rust 1.96.1 toolchain, and a native C linker. Dependencies are vendored. Do not install or run the daemon setuid or with `sudo`.

```sh
RUSTUP_AUTO_INSTALL=0 cargo build --release --offline --locked
install -Dm755 target/release/session-manager "$HOME/.local/bin/session-manager"
export PATH="$HOME/.local/bin${PATH:+:$PATH}"
```

Use real, absolute, lexically normalized paths without symlink ancestors for configuration, project source, state and runtime. PATH entries must also be absolute and normalized, with no empty components. An empty/unset PATH is permitted but supplies no executable-name search for `TryExec`. The daemon snapshots environment locations at startup; restarting is required to change settings or those locations.

## Configure a project source

Choose one existing directory containing projects as immediate subdirectories. It must be disjoint from Session Manager's config/state/runtime trees. Empty project folders are valid; no metadata is required.

Create `${XDG_CONFIG_HOME:-$HOME/.config}/session-manager/config-v2.toml` yourself:

```toml
schema_version = 2
project_source = "/home/YOUR_USER/Projects"
```

Replace the example path with your actual existing directory. The daemon does not create or overwrite this file or the source directory. There is no central profile registry or configuration-edit API.

`XDG_RUNTIME_DIR` must already identify an accessible directory owned by your UID with mode `0700`; no `/tmp` or HOME fallback is guessed. Ordinary Linux user sessions typically supply it. Headless use is supported with an explicitly prepared private runtime directory. Missing state parents are created safely beneath `XDG_STATE_HOME` (default `$HOME/.local/state`); existing parents are not chmodded, replaced or followed through symlinks. Service state/runtime directories are private.

## Run and use

Start the daemon in one terminal:

```sh
session-manager daemon
```

It remains in the foreground. In another terminal:

```sh
session-manager project list
session-manager template list
session-manager --json project create --base-name Work --template "Default Profile"
# Use the project name returned above; Work-1 is only an example for a free first suffix.
session-manager --json session start Work-1
session-manager session list
session-manager --json application list
session-manager session switch UUID_FROM_START
session-manager session stop UUID_FROM_START
```

Replace `UUID_FROM_START` with the returned UUID. Switching does not stop other sessions. Stop removes logical state only, never project files or external applications. No compositor, browser, launcher or adapter needs to be installed. `session-manager --help` lists commands without contacting the daemon.

`--json` may precede the command or follow its complete arguments. It prints exactly the API result object, or an error object with `code`, followed by LF. Command arguments are literal data, not shell commands; quote names containing spaces in your shell. Human output is readable JSON but is not a parsing contract. Exit status is 0 for success/help/version, 1 for startup/API/transport/output errors, and 2 for invalid CLI syntax.

Ctrl-C or SIGTERM requests graceful daemon shutdown: stop accepting connections, drain admitted requests, and release the socket/lock. Logical sessions persist across restart. Do not run two daemons against the same state, or move/delete its runtime directory or lock file while running.

### Templates and application settings

The empty `Default Profile` template is built in; **do not create a filesystem template with that reserved name**. Custom templates are immediate real directories under `${XDG_CONFIG_HOME:-$HOME/.config}/session-manager/profile-template/`. For example, create `profile-template/Notes/README.md`, then select `--template Notes`. Creation chooses the first free numbered project directory, never merges, and applies the [safe-copy limits](technical-specification.md#templates-and-project-creation). Template files are never executed.

Application listing uses the daemon's XDG data roots, not the client's environment. Edit the active project's `.session-manager/apps/exclude.txt` or local `.desktop` files to customize the next query. See [application resolution](technical-specification.md#application-resolution) for the exact formats, precedence, strict validation and bounds. A malformed selected installed or local entry fails the query; it is not silently skipped. Even hidden entries must satisfy the discovery schema. Installed symlinks are skipped; project-local symlinks are rejected. There is no locale translation or exhaustive vendor-extension compatibility guarantee.

Changes to installed/project files are observed on the next request without restarting. Missing projects/no active session yield installed-only apps; same-name replacement directories supply new context. There is no filesystem-wide atomic snapshot or persisted application list.

## API clients

Connect to `$XDG_RUNTIME_DIR/session-manager/ipc-v2.sock` as the daemon owner. Both peers' UIDs and private socket/directory permissions are checked. Each connection carries one LF-terminated UTF-8 JSON request and one response, with no handshake or subscription. The [technical specification](technical-specification.md#public-api-and-cli-version-2) owns method shapes, limits and errors.

Example request (append LF):

```json
{"protocol_version":2,"request_id":"00000000-0000-0000-0000-000000000001","method":"project.list","params":{}}
```

Clients must not bypass the daemon to read/write runtime JSON. A request UUID is correlation, not an idempotence key. Lost replies and timeouts do not cancel a mutation. A nonzero client exit, including an output-write failure, is not proof of rollback. The supplied CLI never automatically retries: `transport.uncertain` means inspect project/session lists before deciding what to do. A `persistence.uncertain` reply likewise does not prove rollback. Repeating start/create can create another independent instance.

This is local single-user coordination, not isolation from malicious code running as the same UID. Resolved Exec/icon/name strings are untrusted client data: never feed Exec to shell `eval`.

## Generic launcher-client example

Python 3 is optional, needed only for this example (and developer checks). [`examples/launcher-client.py`](../examples/launcher-client.py) queries the CLI and emits application objects as JSON Lines:

```sh
python3 examples/launcher-client.py
```

To connect a menu of your choice, explicitly supply a menu bridge command:

```sh
python3 examples/launcher-client.py -- /absolute/path/to/menu-bridge arguments
```

The external command receives one application JSON object per stdin line and returns one JSON object `{"id":"selected.desktop"}` on stdout. The example validates that ID and prints the selected application as JSON. It does **not** execute the application's Exec or require a particular menu program. The menu bridge is a downstream client, not a checkpoint adapter. A launcher that actually starts applications must implement desktop-entry launching semantics itself; shell-evaluating Exec is not a substitute.

## Upgrade, rollback and recovery

Version 2 is an intentional clean break. Supply a new `config-v2.toml`; do not rename a version 1 config/runtime file to a v2 filename. Old configuration, runtime, transactions, adapter registry and checkpoints are ignored and left untouched. They are neither migrated nor required to remain usable. Version 1 is not served concurrently. V2 logical sessions cannot be converted back to v1; reverting an executable is not supported session rollback. No automatic cleanup of old artifacts is performed.

Before replacing a binary, shut down the daemon and keep a backup of the configuration and entire state directory if needed. Do not edit state while the daemon owns it. Upgrades must not silently reinterpret unknown v2 schema fields or reset damaged runtime to empty.

- **`conflict.state_locked`:** another owner holds the state lock. Do not delete the lock file, terminate unrelated processes, or start another instance. Stop your known daemon normally.
- **`conflict.socket_exists`:** startup never unlinks an existing entry, even a stale socket. Verify that your previous daemon has exited and no new instance is starting. Inspect that exact path and ownership; explicitly remove only a confirmed stale `ipc-v2.sock` if appropriate. Do not remove ordinary files/symlinks there as an automatic repair, and do not use existence of `state.lock` as evidence that a process is running.
- **`recovery.invalid_runtime`:** the daemon gates mutations and session/application queries rather than adopting an older revision or guessing empty state. Stop it, preserve current/previous/candidate files, and inspect/repair offline. Restoring a previous revision is an explicit operator decision and may resurrect sessions stopped after that revision; it is never automatic. No force/forget/reset API is provided.
- **`recovery.incomplete_creations`:** an unrecognized staging path remains in the project source. Inspect reserved `.session-manager-create-*` paths explicitly while stopped; the daemon never deletes them automatically on startup. A published numbered project is not a cleanup target merely because a create reply was lost.
- **`application.invalid_*` / `filesystem.unavailable` / `limit.exceeded`:** correct the relevant configured input, accessibility or bounds. The core does not execute files or silently return a partial list to guess around an error.

## Developer checks and safety

```sh
RUSTUP_TOOLCHAIN=1.96.1 CARGO_NET_OFFLINE=true make check
RUSTUP_TOOLCHAIN=1.96.1 make check-isolated
# Or rerun only current test artifacts inside the sandbox:
RUSTUP_TOOLCHAIN=1.96.1 make test-isolated
```

The isolated targets require Linux, bubblewrap, Python 3, GNU make, a native linker and a preinstalled toolchain, with namespace creation permitted. They fail closed if isolation is unavailable. The full check target mounts the toolchain read-only, hides host home/runtime/application/device access, clears environment, disables networking, and uses a separate ignored `target/c3-isolated` build directory. It also builds the release binary offline with the lockfile and checks its version output. The test-only executable override points at this checkout's built binary, never an installed daemon. Only explicitly owned fixture daemons/menu bridges are spawned, signalled and waited for; no live desktop or user state is used.

Initial-release evidence: [C3 acceptance](reviews/c3-acceptance.md). Native desktop integration testing is separate, explicit and not an initial-release dependency.
