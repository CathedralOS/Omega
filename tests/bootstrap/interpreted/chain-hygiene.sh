#!/usr/bin/env sh
# Repository-owner inventory regressions; no compiler execution is needed.
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"

# Reuse the built developer verifier for each copied/archive fixture. Its
# explicit --root selects the fixture; the source construction has no Rust step.
OMEGA_XTASK_BIN=${OMEGA_XTASK_BIN:-"$OMEGA_REPO_ROOT/target/debug/xtask"}
export OMEGA_XTASK_BIN

command -v git >/dev/null 2>&1 || {
  echo "bootstrap owner inventory: skipped (git absent)"
  exit 0
}
command -v python3 >/dev/null 2>&1 || {
  echo "bootstrap owner inventory: skipped (python3 absent)"
  exit 0
}
[ -x "$OMEGA_XTASK_BIN" ] || {
  echo "bootstrap owner inventory: xtask binary is not executable at $OMEGA_XTASK_BIN" >&2
  exit 1
}

FIXTURE_PARENT=$(mktemp -d)
trap 'rm -rf -- "$FIXTURE_PARENT"' EXIT HUP INT TERM
FIXTURE_ROOT="$FIXTURE_PARENT/repository"
mkdir "$FIXTURE_ROOT"
# Snapshot every tracked and untracked non-ignored file, including new source
# members and excluding deletions. No root list is spelled out: a new
# top-level owner tree enters the fixture with the rest of the checkout, so
# the cases below exercise it instead of silently missing it. This keeps the
# gate, path registry, manifests, and their bytes from one state.
python3 - "$OMEGA_REPO_ROOT" "$FIXTURE_ROOT" <<'PY'
import shutil
import subprocess
import sys
from pathlib import Path

source, destination = map(Path, sys.argv[1:])
paths = subprocess.check_output([
    "git", "-C", str(source), "ls-files", "--cached", "--others",
    "--exclude-standard", "-z",
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
  diagnostic=${3:-unexpected product source owners}
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
  if [ "$expected" = rejected ] && ! grep -Fq -- "$diagnostic" "$FIXTURE_PARENT/result"; then
    cat "$FIXTURE_PARENT/result" >&2
    echo "bootstrap owner inventory: $description did not report '$diagnostic'" >&2
    exit 1
  fi
}

# Construction must verify in a checkout without proof experiments, the
# Linux test launcher, or published route reports.
for optional in tests/bootstrap/proofs tests/bootstrap/linux_x86_64 wiki/development/bootstrap
do
  if [ -d "$FIXTURE_ROOT/$optional" ]; then
    mv "$FIXTURE_ROOT/$optional" "$FIXTURE_PARENT/${optional##*/}"
  fi
done
expect_result accepted 'archive without bootstrap experiments'

mkdir -p "$FIXTURE_ROOT/source/retired/empty" \
  "$FIXTURE_ROOT/bootstrap/retired/empty"
expect_result accepted 'archive with empty owner directories'
touch "$FIXTURE_ROOT/source/retired/unexpected.source"
expect_result rejected 'archive with alternate source owner'
rm "$FIXTURE_ROOT/source/retired/unexpected.source"

touch "$FIXTURE_ROOT/bootstrap/retired/unexpected.source"
expect_result rejected 'archive with a route as a source owner' 'unexpected bootstrap owners'
rm "$FIXTURE_ROOT/bootstrap/retired/unexpected.source"
# New implementations belong to the language they execute. Their private
# manifests enter an omission audit when a route declares those inputs.
mkdir -p "$FIXTURE_ROOT/bootstrap/delta/peer"
printf 'member 1 0 ignored omega-rust/peer.seed\n' > "$FIXTURE_ROOT/bootstrap/delta/peer/compiler.sources"
expect_result accepted 'archive with independent bootstrap peer'

for script in check.py run.sh
do
  touch "$FIXTURE_ROOT/bootstrap/delta/peer/$script"
  expect_result rejected "host script in a bootstrap peer" 'host scripts belong outside bootstrap/'
  rm "$FIXTURE_ROOT/bootstrap/delta/peer/$script"
done

for rung in alpha beta
do
  mkdir "$FIXTURE_ROOT/bootstrap/$rung/nested"
  expect_result rejected "nested directory in $rung" 'flat rung contains subdirectories'
  rmdir "$FIXTURE_ROOT/bootstrap/$rung/nested"
done
git -C "$FIXTURE_ROOT" init -q
git -C "$FIXTURE_ROOT" add .
expect_result accepted 'checkout with empty owner directories'
touch "$FIXTURE_ROOT/source/retired/.DS_Store" \
  "$FIXTURE_ROOT/bootstrap/retired/.DS_Store"
expect_result accepted 'checkout with ignored local artifacts'
printf 'peer source\n' > "$FIXTURE_ROOT/bootstrap/delta/peer/new.source"
expect_result accepted 'untracked independent bootstrap peer member'
git -C "$FIXTURE_ROOT" add bootstrap/delta/peer/new.source
expect_result accepted 'staged independent bootstrap peer member'
for tree in source bootstrap
do
  case "$tree" in
    source) owner_error='unexpected product source owners' ;;
    bootstrap) owner_error='unexpected bootstrap owners' ;;
  esac
  touch "$FIXTURE_ROOT/$tree/retired/unexpected.source"
  expect_result rejected "untracked alternate $tree owner" "$owner_error"
  git -C "$FIXTURE_ROOT" add "$tree/retired/unexpected.source"
  expect_result rejected "staged alternate $tree owner" "$owner_error"
  rm "$FIXTURE_ROOT/$tree/retired/unexpected.source"
  expect_result accepted "working copy after removal of alternate $tree owner"
  git -C "$FIXTURE_ROOT" rm -q --cached "$tree/retired/unexpected.source"
done

# The offline launcher consumes the same Delta compiler as the packed closure.
printf 'x' >> "$FIXTURE_ROOT/bootstrap/delta/to-gamma/distribution.gamma"
expect_result rejected 'changed offline Delta distribution' 'Delta compiler distribution'
git -C "$FIXTURE_ROOT" checkout -- bootstrap/delta/to-gamma/distribution.gamma

# Gate-local prefix entries are bound subjects: corrupting one must fail the
# chain gate even though every canonical compiler closure stays byte-exact.
printf 'x' >> "$FIXTURE_ROOT/tests/bootstrap/interpreted/delta/staged-compiler/development_driver.gamma"
expect_result rejected 'modified staged-compiler prefix entry' 'development_driver.gamma'
git -C "$FIXTURE_ROOT" checkout -- tests/bootstrap/interpreted/delta/staged-compiler/development_driver.gamma

printf 'x' >> "$FIXTURE_ROOT/tests/bootstrap/interpreted/omega-parser/main.epsilon"
expect_result rejected 'modified parser-gate prefix entry' 'main.epsilon'
git -C "$FIXTURE_ROOT" checkout -- tests/bootstrap/interpreted/omega-parser/main.epsilon

printf 'x' >> "$FIXTURE_ROOT/tests/bootstrap/conformance/omega/controls_b.epsilon"
expect_result rejected 'modified executable-gate prefix entry' 'controls_b.epsilon'
git -C "$FIXTURE_ROOT" checkout -- tests/bootstrap/conformance/omega/controls_b.epsilon

git -C "$FIXTURE_ROOT" add -f source/retired/.DS_Store
expect_result rejected 'tracked file matching an ignore pattern'
git -C "$FIXTURE_ROOT" rm -q --cached source/retired/.DS_Store
rm "$FIXTURE_ROOT/source/retired/.DS_Store"
expect_result accepted 'clean checkout once the retired owner is removed'

# Rust-producer omission: no produced closure's declared dependency set may
# carry a Rust producer artifact, a toolchain build step or a checkout-derived path.
mkdir -p "$FIXTURE_ROOT/tools/bootstrap/omega"
printf 'cargo build --release\n' > "$FIXTURE_ROOT/tools/bootstrap/omega/forged_env.sh"
expect_result rejected 'bootstrap step invoking the Rust toolchain' \
  'produced closure carries the Rust producer'
rm "$FIXTURE_ROOT/tools/bootstrap/omega/forged_env.sh"
cp "$FIXTURE_ROOT/bootstrap/omega/compiler/omega_compiler.epsilon.sources" \
  "$FIXTURE_PARENT/omega_compiler.epsilon.sources"
for producer in source-rust omega-rust-v2 omega-rust
do
  printf 'cat "%s/forged.epsilon"\n' "$producer" > "$FIXTURE_ROOT/tools/bootstrap/omega/forged_env.sh"
  expect_result rejected "bootstrap step reading $producer/" \
    "producer-build-step: step references Rust producer: $producer/forged.epsilon"
  rm "$FIXTURE_ROOT/tools/bootstrap/omega/forged_env.sh"

  printf 'member 0000000000000000000000000000000000000000000000000000000000000009 1 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 %s/forged.epsilon\n' "$producer" \
    >> "$FIXTURE_ROOT/bootstrap/omega/compiler/omega_compiler.epsilon.sources"
  expect_result rejected "manifest member inside $producer/" \
    "producer-artifact: member is inside $producer/: $producer/forged.epsilon"
  cp "$FIXTURE_PARENT/omega_compiler.epsilon.sources" \
    "$FIXTURE_ROOT/bootstrap/omega/compiler/omega_compiler.epsilon.sources"
done
expect_result accepted 'restored manifests omit the Rust producer again'

echo 'bootstrap owner inventory: archive, checkout, peer, flat-layout, gate-prefix, and Rust-producer omission cases pass'
