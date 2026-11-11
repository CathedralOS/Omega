#!/bin/bash
# Prepare a Linux host so an agent can build and test Omega immediately.
#
# A fresh cloud container has Rust but not the repository's pinned nightly,
# mbx, cargo-nextest or zstd, so the first build and the first `mbx nextest run`
# fail or fall back to slower paths. This script installs each of them once,
# from the documented release channels, and is safe to rerun.
#
#   bash tools/agent_setup.sh             # from anywhere; finds its checkout
#   bash tools/agent_setup.sh --mbx-only  # hosted focused tests need just mbx
#
# Claude Code cloud environment setup script (environment menu -> Edit):
#
#   if [ -d Omega ]; then bash Omega/tools/agent_setup.sh; fi
#
# Tool binaries go to /usr/local/bin as root, else ~/.local/bin. The script
# changes only this checkout's Git config, never the global one.
set -euo pipefail

mbx_only=false
case "$#:${1:-}" in
  0:) ;;
  1:--mbx-only) mbx_only=true ;;
  *) echo "usage: bash tools/agent_setup.sh [--mbx-only]" >&2; exit 2 ;;
esac

# mbx is Mr Boxington, the shared Cargo build cache AGENTS.md builds with:
# https://mr-boxington.jdx.dev/ (source https://github.com/jdx/mr-boxington).
MBX_VERSION=1.29.2
# cargo xtask doctor requires cargo-nextest 0.9.140 or newer.
NEXTEST_VERSION=0.9.150

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
os=$(uname -s)
arch=$(uname -m)
if [ "$arch" = "arm64" ]; then arch="aarch64"; fi
case "$os" in
  Darwin)
    mbx_target="aarch64-apple-darwin"
    nextest_target="universal-apple-darwin"
    ;;
  *)
    mbx_target="$arch-unknown-linux-gnu"
    nextest_target="$arch-unknown-linux-gnu"
    ;;
esac
if [ "$(id -u)" = 0 ]; then bin=/usr/local/bin; else bin=$HOME/.cargo/bin; fi
mkdir -p "$bin"
export PATH="$bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

step() { printf '\n== %s\n' "$*"; }

if [ "$mbx_only" = false ]; then
  step "zstd (unpacks the warm compiler build)"
  if ! command -v zstd >/dev/null; then
    if [ "$(id -u)" = 0 ] && command -v apt-get >/dev/null; then
      # A fresh container has no package lists, so install alone finds nothing.
      { apt-get update -qq && apt-get install -y -qq zstd >/dev/null; } ||
        echo "zstd install failed; tools/warm_build.sh cannot unpack the warm build" >&2
    else
      echo "zstd missing: install it with the host's package manager" >&2
    fi
  fi

  step "pinned Rust toolchain (rust-toolchain.toml)"
  (cd "$repo" && rustup toolchain install && rustc --version)
fi

step "mbx $MBX_VERSION"
if [ "$(mbx --version 2>/dev/null)" != "mbx $MBX_VERSION" ]; then
  archive=mbx-$mbx_target.tar.gz
  release=https://github.com/jdx/mr-boxington/releases/download/v$MBX_VERSION
  (cd "$work" &&
    curl -fsSLO "$release/$archive" && curl -fsSLO "$release/SHA256SUMS" &&
    if command -v sha256sum >/dev/null; then
      grep " $archive\$" SHA256SUMS | sha256sum --check --strict -
    else
      grep " $archive\$" SHA256SUMS | shasum -a 256 --check -
    fi &&
    tar -xzf "$archive" -C "$bin" mbx)
fi
mbx --version
if [ "$mbx_only" = true ]; then
  exit 0
fi
mbx setup --yes || true

step "cargo-nextest $NEXTEST_VERSION"
if ! cargo nextest --version 2>/dev/null | grep -q "^cargo-nextest $NEXTEST_VERSION "; then
  # get.nexte.st is not reachable from cloud sessions; GitHub releases are.
  curl -fsSL "https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-$NEXTEST_VERSION/cargo-nextest-$NEXTEST_VERSION-$nextest_target.tar.gz" |
    tar -xzf - -C "$bin"
fi
cargo nextest --version | head -n 1

step "crates (Cargo.lock)"
(cd "$repo" && mbx fetch --locked)

step "repository settings"
git -C "$repo" config pull.rebase true
# Claude Code reads skills from .claude/skills; the canonical copy is .agents/skills.
if [ ! -e "$repo/.claude/skills" ]; then
  mkdir -p "$repo/.claude" && ln -s ../.agents/skills "$repo/.claude/skills"
fi
# samples/apps/squalr lives in another repository the session may not reach.
git -C "$repo" submodule update --init --recursive ||
  echo "submodule samples/apps/squalr skipped"

step "done; check with: cargo xtask doctor"
