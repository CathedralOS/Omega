#!/bin/bash
# Start Claude Code cloud sessions with the compiler already built.
#
# A cloud session is a fresh 4-CPU container, and building the compiler from
# nothing there takes about 11 minutes. CI on main builds the same compiler in
# an identical container and publishes the build directory as a release asset;
# a session unpacks it before its first build and compiles only what changed
# since. The archive is about 300 MB, so the restore takes seconds.
#
#   bash tools/warm_build.sh pack <slot> <out-dir>   # CI: archive ./target
#   bash tools/warm_build.sh restore <slot>          # unpack into the slot
#   bash tools/warm_build.sh session-start           # SessionStart hook, cloud only
#
# mbx cannot do this job here. This workspace builds with Cargo's checksum
# freshness (.cargo/config.toml), which passes `-Zchecksum-hash-algorithm` to
# rustc, and mbx 1.29.2 does not cache a compilation with a flag it does not
# model: every unit is reported as bypassed and its store stays empty. The
# same setting is what makes a plain directory restore sound: Cargo compares
# source contents, not file times, so a fresh clone reuses a restored build and
# recompiles exactly the crates whose sources differ.
#
# There are two slots because Cargo gives the two places a session builds in
# different identities. A pool worktree (.agents/worktree/worker-N) sits inside
# the primary checkout, so Cargo reads both .cargo/config.toml files and passes
# the rustflags twice; a build made in one place is never reused in the other.
#   primary  the checkout itself, where `cargo xtask worktree acquire` first runs
#   worker   .agents/worktree/worker-1, the first pool slot
# CI builds each slot in a container set up like a session: as root, at the
# same paths, with the tools tools/agent_setup.sh installs.
#
# The restore pays off only at the published commit. Most of the 11 minutes is
# this workspace's own 89 crates, and a change to a foundation crate such as
# language-core recompiles all of them: a build restored three commits away
# from its sources took 673 s, as long as a cold one. session-start therefore
# prints the published commit and whether the checkout's compiler sources
# still match it.
set -euo pipefail

REPOSITORY=CathedralOS/Omega
TAG=warm-build-linux-x86_64
# A local directory served as file:// stands in for the release in tests.
SOURCE=${OMEGA_WARM_BUILD_SOURCE:-https://github.com/$REPOSITORY/releases/download/$TAG}
SCHEMA=omega-warm-build-v2
# Room the first build needs beyond the unpacked archive.
HEADROOM_KIB=$((2 * 1024 * 1024))
# What `mbx build -p omega` compiles: a difference elsewhere rebuilds nothing.
COMPILED="omega-rust source/library Cargo.toml Cargo.lock rust-toolchain.toml .cargo"

fail() { echo "warm build: $*" >&2; exit 1; }

primary_root() {
  local common
  common=$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --path-format=absolute --git-common-dir)
  dirname "$common"
}

slot_dir() {
  case "$1" in
    primary) primary_root ;;
    worker) echo "$(primary_root)/.agents/worktree/worker-1" ;;
    *) fail "unknown slot '$1' (primary or worker)" ;;
  esac
}

# The exact compiler: artifacts from any other one are rebuilt, never reused.
toolchain() { (cd "$1" && rustc -V); }

manifest_value() { sed -n "s/^$2=//p" "$1" | head -n 1; }

pack() {
  local slot=$1 out=$2 dir commit asset
  dir=$(slot_dir "$slot")
  [ "$(pwd -P)" = "$dir" ] || fail "pack $slot runs in $dir, the path sessions build at"
  [ -x target/debug/omega ] || fail "target/debug/omega is missing; build before packing"
  commit=$(git rev-parse HEAD)
  asset=target-$slot-${commit:0:12}.tar.zst
  mkdir -p "$out"
  # pax headers keep nanosecond file times; Cargo compares outputs by them.
  tar --format=posix -cf - target | zstd -T0 -6 -q -f -o "$out/$asset"
  {
    echo "schema=$SCHEMA"
    echo "slot=$slot"
    echo "commit=$commit"
    echo "rustc=$(toolchain "$dir")"
    echo "asset=$asset"
    echo "sha256=$(sha256sum "$out/$asset" | cut -d' ' -f1)"
    echo "unpacked_kib=$(du -sk target | cut -f1)"
  } > "$out/warm-build-$slot.txt"
  cat "$out/warm-build-$slot.txt"
}

fetch() {
  # Release downloads through the session proxy are sometimes cut off partway;
  # -C - resumes at the next unread byte instead of starting over.
  curl -fsSL --retry 5 --retry-all-errors --retry-delay 2 --connect-timeout 20 -C - -o "$2" "$SOURCE/$1"
}

restore() (
  local slot=$1 root dir stage manifest commit asset want have need free started
  started=$(date +%s)
  root=$(primary_root)
  dir=$(slot_dir "$slot")
  if [ ! -e "$dir" ]; then
    # The pool's first slot may not exist yet. `cargo xtask worktree acquire`
    # prefers an idle slot that already holds a build, so it hands out this one.
    git -C "$root" worktree add -q --detach "$dir" HEAD
  fi
  # Never unpack over a build: another restore or a session's own build owns it.
  if [ -n "$(ls -A "$dir/target" 2>/dev/null)" ]; then
    echo "warm build: $slot kept, $dir/target already holds a build"
    return 0
  fi
  stage=$root/.agents/worktree/.warm-$slot.$$
  mkdir -p "$stage"
  trap 'rm -rf "$stage"' EXIT
  manifest=$stage/manifest.txt
  fetch "warm-build-$slot.txt" "$manifest" || fail "$slot: no published build at $SOURCE"
  [ "$(manifest_value "$manifest" schema)" = "$SCHEMA" ] || fail "$slot: unknown manifest schema"
  commit=$(manifest_value "$manifest" commit)
  asset=$(manifest_value "$manifest" asset)
  want=$(manifest_value "$manifest" rustc)
  have=$(toolchain "$dir")
  [ "$want" = "$have" ] || fail "$slot: built with '$want', this checkout uses '$have'"
  need=$(($(manifest_value "$manifest" unpacked_kib) + HEADROOM_KIB))
  free=$(df -Pk "$dir" | awk 'NR==2 {print $4}')
  [ "$free" -ge "$need" ] || fail "$slot: needs $need KiB free, the disk has $free KiB"
  fetch "$asset" "$stage/$asset" || fail "$slot: download of $asset failed"
  echo "$(manifest_value "$manifest" sha256)  $stage/$asset" | sha256sum --check --status ||
    fail "$slot: $asset does not match its published digest"
  zstd -dc "$stage/$asset" | tar -xf - -C "$stage"
  cp "$manifest" "$stage/target/.warm-build.txt"
  # One rename publishes the whole build. It fails if a build appeared in the
  # meantime, and that build then stays untouched.
  rmdir "$dir/target" 2>/dev/null || true
  mv -T "$stage/target" "$dir/target" 2>/dev/null ||
    fail "$slot: $dir/target appeared during the restore; left as it is"
  echo "warm build: $slot restored commit ${commit:0:12} into $dir/target in $(($(date +%s) - started)) s"
)

session_start() {
  # Other hosts keep their builds between sessions and have nothing to restore.
  [ "${CLAUDE_CODE_REMOTE:-}" = "true" ] || return 0
  # mbx deletes build directories, mid-build included, when free space falls
  # under 10% of the disk's size. The container's disk reports 252 GB while the
  # session may use about 30 GB of it, so that floor is 25 GiB and one compiler
  # build crosses it. 5 GiB is mbx's own minimum for a small disk.
  if command -v mbx >/dev/null && [ -z "$(mbx settings get gc.min_free_size 2>/dev/null)" ]; then
    mbx settings set gc.min_free_size 5GiB >/dev/null 2>&1 || true
  fi
  local slot
  for slot in primary worker; do
    restore "$slot" 2>&1 || echo "warm build: $slot not restored; the first build compiles everything"
  done
  local stamp=$(primary_root)/.agents/worktree/worker-1/target/.warm-build.txt commit
  [ -f "$stamp" ] || return 0
  commit=$(manifest_value "$stamp" commit)
  if git -C "$(primary_root)" diff --quiet "$commit" HEAD -- $COMPILED 2>/dev/null; then
    echo "warm build: this checkout's compiler sources match it; the first build takes under a minute"
  else
    echo "warm build: compiler sources here differ from commit ${commit:0:12}, so the first build recompiles" \
      "what changed; to start from a built compiler, base new work on that commit" \
      "(git checkout --detach ${commit:0:12} in the acquired worktree)"
  fi
}

case "${1:-}:$#" in
  pack:3) pack "$2" "$3" ;;
  restore:2) restore "$2" ;;
  # A hook must never block or fail a session; the cold build still works.
  session-start:1) session_start || true ;;
  *) fail "usage: warm_build.sh pack <slot> <out-dir> | restore <slot> | session-start" ;;
esac
