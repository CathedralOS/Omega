#!/usr/bin/env sh
# Repository-owner inventory regressions; no compiler execution is needed.
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"

command -v git >/dev/null 2>&1 || {
  echo "bootstrap owner inventory: skipped (git absent)"
  exit 0
}
command -v python3 >/dev/null 2>&1 || {
  echo "bootstrap owner inventory: skipped (python3 absent)"
  exit 0
}

FIXTURE_PARENT=$(mktemp -d)
trap 'rm -rf -- "$FIXTURE_PARENT"' EXIT HUP INT TERM
FIXTURE_ROOT="$FIXTURE_PARENT/repository"
mkdir "$FIXTURE_ROOT"
# Snapshot current files, including new source members and excluding deletions.
# This keeps the gate, path registry, manifests, and their bytes from one state.
python3 - "$OMEGA_REPO_ROOT" "$FIXTURE_ROOT" <<'PY'
import shutil
import subprocess
import sys
from pathlib import Path

source, destination = map(Path, sys.argv[1:])
paths = subprocess.check_output([
    "git", "-C", str(source), "ls-files", "--cached", "--others",
    "--exclude-standard", "-z", "--", ".gitignore", "bootstrap", "source",
    "tests", "tools", "wiki", "README.md", "TASKS_BOOTSTRAP.md",
])
for spelling in set(paths.split(b"\0")) - {b""}:
    relative = Path(spelling.decode())
    original = source / relative
    copied = destination / relative
    if original.is_file() or original.is_symlink():
        copied.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original, copied, follow_symlinks=False)
PY

expect_result() {
  expected=$1
  description=$2
  diagnostic=${3:-owners differ}
  if sh "$FIXTURE_ROOT/tools/bootstrap/check-chain-hygiene.sh" \
      > "$FIXTURE_PARENT/result" 2>&1; then
    actual=accepted
  else
    actual=rejected
  fi
  if [ "$actual" != "$expected" ]; then
    cat "$FIXTURE_PARENT/result" >&2
    echo "bootstrap owner inventory: $description was $actual" >&2
    exit 1
  fi
  if [ "$expected" = rejected ]; then
    grep -q "$diagnostic" "$FIXTURE_PARENT/result"
  fi
}

mkdir -p "$FIXTURE_ROOT/source/retired/empty" \
  "$FIXTURE_ROOT/bootstrap/retired/empty"
expect_result accepted 'archive with empty owner directories'
for tree in source bootstrap
do
  touch "$FIXTURE_ROOT/$tree/retired/unexpected.source"
  expect_result rejected "archive with alternate $tree owner"
  rm "$FIXTURE_ROOT/$tree/retired/unexpected.source"
done

for rung in 0_alpha 1_beta 2_gamma
do
  mkdir "$FIXTURE_ROOT/bootstrap/$rung/nested"
  expect_result rejected "nested directory in $rung" 'flat rung contains subdirectories'
  rmdir "$FIXTURE_ROOT/bootstrap/$rung/nested"
done
for rung in 3_delta 4_epsilon 5_omega
do
  mkdir "$FIXTURE_ROOT/bootstrap/$rung/compiler"
  expect_result rejected "redundant compiler directory in $rung" 'redundant compiler directory remains'
  rmdir "$FIXTURE_ROOT/bootstrap/$rung/compiler"
done

git -C "$FIXTURE_ROOT" init -q
git -C "$FIXTURE_ROOT" add .
expect_result accepted 'checkout with empty owner directories'
touch "$FIXTURE_ROOT/source/retired/.DS_Store" \
  "$FIXTURE_ROOT/bootstrap/retired/.DS_Store"
expect_result accepted 'checkout with ignored local artifacts'
for tree in source bootstrap
do
  touch "$FIXTURE_ROOT/$tree/retired/unexpected.source"
  expect_result rejected "untracked alternate $tree owner"
  git -C "$FIXTURE_ROOT" add "$tree/retired/unexpected.source"
  expect_result rejected "staged alternate $tree owner"
  git -C "$FIXTURE_ROOT" rm -q --cached "$tree/retired/unexpected.source"
  rm "$FIXTURE_ROOT/$tree/retired/unexpected.source"
done
# Gate-local prefix entries are bound subjects: corrupting one must fail the
# chain gate even though every canonical compiler closure stays byte-exact.
printf 'x' >> "$FIXTURE_ROOT/tests/delta/staged-compiler/development_driver.gamma"
expect_result rejected 'modified staged-compiler prefix entry' 'development_driver.gamma'
git -C "$FIXTURE_ROOT" checkout -- tests/delta/staged-compiler/development_driver.gamma

printf 'x' >> "$FIXTURE_ROOT/tests/bootstrap/omega-parser/main.epsilon"
expect_result rejected 'modified parser-gate prefix entry' 'main.epsilon'
git -C "$FIXTURE_ROOT" checkout -- tests/bootstrap/omega-parser/main.epsilon

printf 'x' >> "$FIXTURE_ROOT/tests/bootstrap/omega-executable/controls_b.epsilon"
expect_result rejected 'modified executable-gate prefix entry' 'controls_b.epsilon'
git -C "$FIXTURE_ROOT" checkout -- tests/bootstrap/omega-executable/controls_b.epsilon

git -C "$FIXTURE_ROOT" add -f source/retired/.DS_Store
expect_result rejected 'tracked file matching an ignore pattern'
git -C "$FIXTURE_ROOT" rm -q --cached source/retired/.DS_Store
rm "$FIXTURE_ROOT/source/retired/.DS_Store"
expect_result accepted 'clean checkout once the retired owner is removed'

# Rust-producer omission: no produced closure's declared dependency set may
# carry an omega-rust/ artifact, a Rust-toolchain build step, or a
# checkout-derived path (omega-rust/README.md).
mkdir -p "$FIXTURE_ROOT/tools/bootstrap/omega"
printf 'cargo build --release\n' > "$FIXTURE_ROOT/tools/bootstrap/omega/forged_env.sh"
expect_result rejected 'bootstrap step invoking the Rust toolchain' \
  'produced closure carries the Rust producer'
rm "$FIXTURE_ROOT/tools/bootstrap/omega/forged_env.sh"
cp "$FIXTURE_ROOT/bootstrap/5_omega/omega_compiler.epsilon.sources" \
  "$FIXTURE_PARENT/omega_compiler.epsilon.sources"
printf 'member 0000000000000000000000000000000000000000000000000000000000000009 1 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 omega-rust/forged.epsilon\n' \
  >> "$FIXTURE_ROOT/bootstrap/5_omega/omega_compiler.epsilon.sources"
expect_result rejected 'manifest member inside omega-rust/' \
  'produced closure carries the Rust producer'
cp "$FIXTURE_PARENT/omega_compiler.epsilon.sources" \
  "$FIXTURE_ROOT/bootstrap/5_omega/omega_compiler.epsilon.sources"
expect_result accepted 'restored manifests omit the Rust producer again'

echo 'bootstrap owner inventory: 23 archive, checkout, flat-layout, gate-prefix, and Rust-producer omission cases pass'
