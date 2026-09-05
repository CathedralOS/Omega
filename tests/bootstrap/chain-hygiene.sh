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
    grep -q 'owners differ' "$FIXTURE_PARENT/result"
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
git -C "$FIXTURE_ROOT" add -f source/retired/.DS_Store
expect_result rejected 'tracked file matching an ignore pattern'

echo 'bootstrap owner inventory: 10 archive and checkout cases pass'
