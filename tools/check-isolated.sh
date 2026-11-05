#!/usr/bin/env bash
# Run the entire release check in a private, network-disabled environment.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
repo=$PWD
command -v bwrap >/dev/null
export RUSTUP_AUTO_INSTALL=0
toolchain=$(rustc --print sysroot)
mkdir -p -- target/c3-isolated
bwrap --die-with-parent --ro-bind / / \
    --tmpfs /home --tmpfs /root --tmpfs /run --tmpfs /tmp --dev /dev \
    --tmpfs /usr/share/applications --tmpfs /usr/local/share \
    --ro-bind "$repo" /tmp/work --bind "$repo/target/c3-isolated" /tmp/work/target \
    --ro-bind "$toolchain" /tmp/toolchain --chdir /tmp/work \
    --unshare-net --unshare-pid --proc /proc --clearenv \
    --setenv HOME /tmp/build-home --setenv CARGO_HOME /tmp/build-home/.cargo \
    --setenv PATH /tmp/toolchain/bin:/usr/bin:/bin --setenv CARGO_NET_OFFLINE true \
    --setenv SESSION_MANAGER_TEST_BINARY /tmp/work/target/debug/session-manager \
    --setenv XDG_CONFIG_HOME /tmp/build-home/config \
    --setenv XDG_STATE_HOME /tmp/build-home/state \
    --setenv XDG_RUNTIME_DIR /tmp/build-home/runtime \
    --setenv XDG_DATA_HOME /tmp/build-home/data \
    --setenv XDG_DATA_DIRS /tmp/build-home/data-dirs \
    /bin/sh -c 'mkdir -p "$HOME"; make check && cargo build --release --offline --locked && target/release/session-manager --version'
