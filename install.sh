#!/bin/sh
# install.sh — agent-cli installer
#
# Usage:
#   sh install.sh
#   curl -fsSL https://raw.githubusercontent.com/<owner>/agent-cli/main/install.sh | sh
#
# Environment variables:
#   AGENT_CLI_REPO          source repository URL (default: GitHub source)
#   AGENT_CLI_REF           branch/tag/commit to check out (default: main)
#   AGENT_CLI_PREFIX        install prefix (default: $HOME/.local)
#   AGENT_CLI_INSTALL_FORCE if set to 1, allow overwriting an existing binary

set -eu

AGENT_CLI_REPO=${AGENT_CLI_REPO:-https://github.com/example/agent-cli.git}
AGENT_CLI_REF=${AGENT_CLI_REF:-main}
AGENT_CLI_PREFIX=${AGENT_CLI_PREFIX:-"$HOME/.local"}
AGENT_CLI_INSTALL_FORCE=${AGENT_CLI_INSTALL_FORCE:-0}

log() {
    printf '[install.sh] %s\n' "$*"
}

err() {
    printf '[install.sh] error: %s\n' "$*" >&2
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        err "$1 is required but not found."
        case "$1" in
            cargo|rustc)
                err "Install Rust toolchain first:"
                err "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                ;;
            git)
                err "Install git via your distro package manager (e.g. apt install git)."
                ;;
        esac
        exit 1
    fi
}

# ---------- platform check ----------
os="$(uname -s 2>/dev/null || echo unknown)"
arch="$(uname -m 2>/dev/null || echo unknown)"

if [ "$os" != "Linux" ]; then
    err "agent-cli currently supports Linux only (got: $os)."
    exit 1
fi
case "$arch" in
    x86_64|amd64|aarch64|arm64)
        ;;
    *)
        err "Unsupported architecture: $arch (expected x86_64 or aarch64)."
        exit 1
        ;;
esac

log "platform: Linux/$arch"

# ---------- prerequisites ----------
require_cmd cargo
require_cmd uname
require_cmd mkdir
require_cmd rm

# ---------- decide source location ----------
script_dir=""
if [ -n "${BASH_SOURCE:-}" ]; then
    script_dir=$(cd "$(dirname "${BASH_SOURCE:-$0}")" 2>/dev/null && pwd) || script_dir=""
elif [ -n "${0:-}" ] && [ -f "$0" ]; then
    script_dir=$(cd "$(dirname "$0")" 2>/dev/null && pwd) || script_dir=""
fi

is_repo_root() {
    # Check for our Cargo.toml at the given dir
    [ -f "$1/Cargo.toml" ] && grep -q '^name = "agent-cli"' "$1/Cargo.toml" 2>/dev/null
}

source_dir=""
cleanup_dir=""

if is_repo_root "$(pwd)"; then
    source_dir="$(pwd)"
    log "using local source: $source_dir"
elif [ -n "$script_dir" ] && is_repo_root "$script_dir"; then
    source_dir="$script_dir"
    log "using script-relative source: $source_dir"
else
    require_cmd git
    require_cmd mktemp
    tmp=$(mktemp -d 2>/dev/null || mktemp -d -t agent-cli-install)
    cleanup_dir="$tmp"
    log "cloning $AGENT_CLI_REPO @ $AGENT_CLI_REF into $tmp"
    git clone --depth 1 --branch "$AGENT_CLI_REF" "$AGENT_CLI_REPO" "$tmp/src" \
        || git clone "$AGENT_CLI_REPO" "$tmp/src" \
        || { err "git clone failed."; exit 1; }
    if [ "$AGENT_CLI_REF" != "main" ]; then
        ( cd "$tmp/src" && git checkout "$AGENT_CLI_REF" ) || true
    fi
    if ! is_repo_root "$tmp/src"; then
        err "cloned source does not look like agent-cli (no matching Cargo.toml)."
        exit 1
    fi
    source_dir="$tmp/src"
fi

# ---------- target check ----------
bin_dir="$AGENT_CLI_PREFIX/bin"
target_bin="$bin_dir/agent-cli"
mkdir -p "$bin_dir"

if [ -e "$target_bin" ] && [ "$AGENT_CLI_INSTALL_FORCE" != "1" ]; then
    log "existing binary detected: $target_bin"
    log "overwriting (set AGENT_CLI_INSTALL_FORCE=1 to suppress this message)"
fi

# ---------- build & install ----------
log "building and installing into $AGENT_CLI_PREFIX"
( cd "$source_dir" && cargo install --path . --root "$AGENT_CLI_PREFIX" --force --locked ) \
    || ( cd "$source_dir" && cargo install --path . --root "$AGENT_CLI_PREFIX" --force ) \
    || { err "cargo install failed."; exit 1; }

# ---------- cleanup ----------
if [ -n "$cleanup_dir" ] && [ -d "$cleanup_dir" ]; then
    rm -rf "$cleanup_dir"
fi

# ---------- post-install ----------
log "installed: $target_bin"
"$target_bin" --version >/dev/null 2>&1 \
    && log "version check: $($target_bin --version)" \
    || log "version check: failed (binary may still be functional)"

case ":$PATH:" in
    *":$bin_dir:"*)
        log "PATH ok ($bin_dir is on PATH)"
        ;;
    *)
        log "warning: $bin_dir is NOT on PATH"
        log "  add the following to your shell profile:"
        log "    export PATH=\"$bin_dir:\$PATH\""
        ;;
esac

log "done. try: agent-cli --help"
