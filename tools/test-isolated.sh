#!/usr/bin/env bash
# Build offline, then run only current Cargo test artifacts without host session access.
set -euo pipefail
export RUSTUP_AUTO_INSTALL=0
cd -- "$(dirname -- "$0")/.."
repo=$PWD
command -v bwrap >/dev/null
command -v python3 >/dev/null
manifest=$(mktemp)
trap 'rm -f -- "$manifest"' EXIT
CARGO_NET_OFFLINE=true cargo test --all-targets --all-features --no-run --message-format=json > "$manifest"
mapfile -t executables < <(python3 - "$manifest" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    for line in stream:
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message.get("profile", {}).get("test") and message.get("executable"):
            print(message["executable"])
PY
)
((${#executables[@]} > 0))
for executable in "${executables[@]}"; do
    relative=${executable#"$repo"/}
    [[ "$relative" != "$executable" && "$relative" != ../* ]]
    bwrap --die-with-parent --ro-bind / / \
        --tmpfs /home --tmpfs /root --tmpfs /run --tmpfs /tmp --dev /dev \
        --tmpfs /usr/share/applications --tmpfs /usr/local/share \
        --ro-bind "$repo" /tmp/work --chdir /tmp/work \
        --unshare-net --unshare-pid --proc /proc --clearenv \
        --setenv PATH /usr/bin:/bin \
        --setenv SESSION_MANAGER_TEST_BINARY /tmp/work/target/debug/session-manager \
        --setenv HOME /tmp/isolated-home \
        --setenv XDG_CONFIG_HOME /tmp/isolated-home/config \
        --setenv XDG_STATE_HOME /tmp/isolated-home/state \
        --setenv XDG_RUNTIME_DIR /tmp/isolated-home/runtime \
        --setenv XDG_DATA_HOME /tmp/isolated-home/data \
        --setenv XDG_DATA_DIRS /tmp/isolated-home/data-dirs \
        /bin/sh -c 'mkdir -p "$HOME"; exec "$1"' sh "/tmp/work/$relative"
done
