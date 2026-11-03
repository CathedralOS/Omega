#!/bin/bash
# Omega — Claude Code cloud environment setup script.
# Paste into: environment menu (session title bar) -> Edit -> Setup script.
# Runs as root on Ubuntu 24.04 before Claude starts. Must exit 0 and finish in
# about 5 minutes; the result is snapshotted and reused for ~7 days. A script
# that runs longer is not cached and runs again at every session start.
export PATH="$HOME/.cargo/bin:$PATH"
REPO=/home/user/Omega
TOOLCHAIN=nightly-2026-09-04   # fallback; rust-toolchain.toml wins when present
if [ -f "$REPO/rust-toolchain.toml" ]; then
  TOOLCHAIN=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$REPO/rust-toolchain.toml")
fi
NEXTEST=0.9.150

# 1. zstd unpacks the warm compiler build that the repo's SessionStart hook
#    restores into .agents/worktree/warm (tools/warm_build.py).
(apt-get update -qq && apt-get install -y -qq zstd) &

# 2. Pinned Rust toolchain (clippy + rustfmt), then pre-download every crate.
(
  rustup toolchain install "$TOOLCHAIN" --profile minimal -c clippy -c rustfmt
  if [ -f "$REPO/Cargo.lock" ]; then
    cd "$REPO" && cargo +"$TOOLCHAIN" fetch --locked
  fi
) &

# 3. cargo-nextest >= 0.9.140. get.nexte.st is blocked by the cloud proxy, but
#    GitHub release downloads are allowed: fetch the prebuilt binary (seconds)
#    and fall back to building from crates.io (minutes) only if that fails.
(
  url="https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-$NEXTEST/cargo-nextest-$NEXTEST-x86_64-unknown-linux-gnu.tar.gz"
  curl -fsSL "$url" | tar zxf - -C "$HOME/.cargo/bin" \
    || cargo +stable install cargo-nextest --locked
) &

# 4. Repository pieces, when the checkout is already present.
if [ -d "$REPO/.git" ]; then
  # Squalr sample submodule (samples/apps/squalr), best effort.
  git -C "$REPO" submodule update --init --recursive &

  # Expose .agents/skills to Claude Code (/advance, /claude-swarm, ...).
  if [ ! -e "$REPO/.claude/skills" ]; then
    mkdir -p "$REPO/.claude" && ln -s ../.agents/skills "$REPO/.claude/skills"
  fi
  # User-level links survive the fresh clone that cached sessions get.
  mkdir -p /root/.claude/skills
  for skill in "$REPO"/.agents/skills/*/; do
    ln -sfn "${skill%/}" "/root/.claude/skills/$(basename "$skill")"
  done
fi

# 5. AGENTS.md: keep history linear on pull.
git config --system pull.rebase true

wait
cargo nextest --version || echo "cargo-nextest missing"
zstd --version || echo "zstd missing"
exit 0
