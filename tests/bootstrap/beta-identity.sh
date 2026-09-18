#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

require_beta_compiler_identity ||
  fail "bound identity check refused the canonical checkout"
echo "canonical: audited source and tape pass the bound identity check"

materialize_beta_compiler "$TMP/compiler" ||
  fail "materialization of the audited tape failed"
{ printf '\355\006\000\000'; cat "$OMEGA_PATH_BETA_COMPILER_TAPE"; } > "$TMP/expected"
tape_in_seed "$TMP/compiler" > "$TMP/stamped"
cmp -s "$TMP/stamped" "$TMP/expected" ||
  fail "stamped [len][tape] payload differs from the audited tape"
echo "materialize: stamped seed embeds exactly [len][audited tape]"

cp "$OMEGA_PATH_BETA_COMPILER_SOURCE" "$TMP/corrupt-source.beta"
if [ "$(od -An -tc -j 100 -N1 "$TMP/corrupt-source.beta" | tr -d ' ')" = "a" ]; then
  printf 'b' | dd of="$TMP/corrupt-source.beta" bs=1 seek=100 conv=notrunc status=none
else
  printf 'a' | dd of="$TMP/corrupt-source.beta" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_BETA_COMPILER_SOURCE=$TMP/corrupt-source.beta
  . "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
  materialize_beta_compiler "$TMP/refused-source"
) 2>"$TMP/corrupt-source.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted source: expected exit 3, got $rc"
grep -q 'AUDIT.md' "$TMP/corrupt-source.err" ||
  fail "corrupted source: refusal did not cite AUDIT.md"
[ ! -e "$TMP/refused-source" ] ||
  fail "corrupted source: destination was written"
echo "corrupt: a one-byte source change is refused before stamping"

head -c $((BETA_COMPILER_SOURCE_SIZE - 1)) \
  "$OMEGA_PATH_BETA_COMPILER_SOURCE" > "$TMP/truncated-source.beta"
rc=0
(
  export OMEGA_PATH_BETA_COMPILER_SOURCE=$TMP/truncated-source.beta
  . "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
  materialize_beta_compiler "$TMP/refused-source-truncated"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated source: expected exit 3, got $rc"
[ ! -e "$TMP/refused-source-truncated" ] ||
  fail "truncated source: destination was written"
echo "truncate: a truncated source is refused before stamping"

cp "$OMEGA_PATH_BETA_COMPILER_TAPE" "$TMP/corrupt.tape"
if [ "$(od -An -tx1 -j 100 -N1 "$TMP/corrupt.tape" | tr -d ' ')" = "ff" ]; then
  printf '\000' | dd of="$TMP/corrupt.tape" bs=1 seek=100 conv=notrunc status=none
else
  printf '\377' | dd of="$TMP/corrupt.tape" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_BETA_COMPILER_TAPE=$TMP/corrupt.tape
  . "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
  materialize_beta_compiler "$TMP/refused"
) 2>"$TMP/corrupt.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted tape: expected exit 3, got $rc"
grep -q 'AUDIT.md' "$TMP/corrupt.err" ||
  fail "corrupted tape: refusal did not cite AUDIT.md"
[ ! -e "$TMP/refused" ] ||
  fail "corrupted tape: destination was written"
echo "corrupt: a one-byte tape change is refused before stamping"

head -c 1772 "$OMEGA_PATH_BETA_COMPILER_TAPE" > "$TMP/truncated.tape"
rc=0
(
  export OMEGA_PATH_BETA_COMPILER_TAPE=$TMP/truncated.tape
  . "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
  materialize_beta_compiler "$TMP/refused-truncated"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated tape: expected exit 3, got $rc"
[ ! -e "$TMP/refused-truncated" ] ||
  fail "truncated tape: destination was written"
echo "truncate: a truncated tape is refused before stamping"

for needle in \
  "$BETA_COMPILER_SOURCE_SHA256" "$BETA_COMPILER_TAPE_SHA256" "12,536" "1,773"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/1_beta/AUDIT.md" ||
    fail "AUDIT.md lacks bound record $needle"
done
for needle in \
  "$BETA_COMPILER_SOURCE_SHA256" "$BETA_COMPILER_TAPE_SHA256" "12,536" "1,773"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/beta/compiler/root-audit.py" ||
    fail "root-audit.py lacks bound record $needle"
done
for needle in "12,536" "1,773"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/1_beta/LANGUAGE.md" ||
    fail "bootstrap/1_beta/LANGUAGE.md lacks bound record $needle"
done
grep -q "1,773" "$OMEGA_REPO_ROOT/bootstrap/1_beta/README.md" ||
  fail "bootstrap/1_beta/README.md lacks bound tape record"
echo "records: bound identities match AUDIT.md, root-audit.py, LANGUAGE.md, and the rung README"

echo "Beta identity: bound subject stamped exactly; corrupted and truncated sources and tapes refused"
