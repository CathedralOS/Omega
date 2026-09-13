#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "Delta identity: skipped (python3 absent)"
  exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

require_delta_compiler_identity ||
  fail "bound identity check refused the canonical checkout"
echo "canonical: bound entry, manifest, composed record, and closure pass"

materialize_delta_compiler "$TMP/compiler.gamma" ||
  fail "materialization of the bound closure failed"
require_bound_identity "materialized compiler" "$TMP/compiler.gamma" \
  "$DELTA_COMPILER_PACKED_SIZE" "$DELTA_COMPILER_PACKED_SHA256" \
  "bootstrap/3_delta/delta_compiler.composed" ||
  fail "materialized closure differs from the bound identity"
head -c "$DELTA_COMPILER_ENTRY_SIZE" "$TMP/compiler.gamma" |
  cmp -s - "$OMEGA_PATH_DELTA_COMPILER_SOURCE" ||
  fail "materialized prefix differs from the canonical entry"
echo "materialize: packed closure is exactly the bound entry-plus-member bytes"

cp "$OMEGA_PATH_DELTA_COMPILER_SOURCE" "$TMP/corrupt-entry.gamma"
if [ "$(od -An -tx1 -j 100 -N1 "$TMP/corrupt-entry.gamma" | tr -d ' ')" = "ff" ]; then
  printf '\000' | dd of="$TMP/corrupt-entry.gamma" bs=1 seek=100 conv=notrunc status=none
else
  printf '\377' | dd of="$TMP/corrupt-entry.gamma" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_DELTA_COMPILER_SOURCE=$TMP/corrupt-entry.gamma
  materialize_delta_compiler "$TMP/refused-entry"
) 2>"$TMP/corrupt-entry.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted entry: expected exit 3, got $rc"
[ ! -e "$TMP/refused-entry" ] ||
  fail "corrupted entry: destination was written"
echo "corrupt: a one-byte entry change is refused before packing"

cp -R "$OMEGA_PATH_DELTA_COMPILER/implementation" "$TMP/implementation"
head -c $((DELTA_COMPILER_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_DELTA_COMPILER_SOURCES" > "$TMP/implementation/implementation.gamma.sources"
rc=0
(
  export OMEGA_PATH_DELTA_COMPILER_SOURCES=$TMP/implementation/implementation.gamma.sources
  materialize_delta_compiler "$TMP/refused-truncated"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated manifest: expected exit 3, got $rc"
[ ! -e "$TMP/refused-truncated" ] ||
  fail "truncated manifest: destination was written"
echo "truncate: a truncated manifest is refused before packing"

cp "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$TMP/implementation/implementation.gamma.sources"
CORRUPT_MEMBER="$TMP/implementation/lowering/bindings.gamma"
if [ "$(od -An -tc -j 100 -N1 "$CORRUPT_MEMBER" | tr -d ' ')" = "a" ]; then
  printf 'b' | dd of="$CORRUPT_MEMBER" bs=1 seek=100 conv=notrunc status=none
else
  printf 'a' | dd of="$CORRUPT_MEMBER" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_DELTA_COMPILER_SOURCES=$TMP/implementation/implementation.gamma.sources
  materialize_delta_compiler "$TMP/refused-member"
) 2>"$TMP/corrupt-member.err" || rc=$?
[ "$rc" != 0 ] ||
  fail "corrupted member: materialization unexpectedly succeeded"
grep -q 'member digest changed' "$TMP/corrupt-member.err" ||
  fail "corrupted member: refusal did not name the member digest"
[ ! -e "$TMP/refused-member" ] ||
  fail "corrupted member: destination was written"
echo "member: a one-byte member change is refused during packing"

cp "$OMEGA_PATH_DELTA_COMPILER_COMPOSED" "$TMP/corrupt.composed"
if [ "$(od -An -tx1 -j 40 -N1 "$TMP/corrupt.composed" | tr -d ' ')" = "ff" ]; then
  printf '\000' | dd of="$TMP/corrupt.composed" bs=1 seek=40 conv=notrunc status=none
else
  printf '\377' | dd of="$TMP/corrupt.composed" bs=1 seek=40 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_DELTA_COMPILER_COMPOSED=$TMP/corrupt.composed
  materialize_delta_compiler "$TMP/refused-composed"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted composed record: expected exit 3, got $rc"
[ ! -e "$TMP/refused-composed" ] ||
  fail "corrupted composed record: destination was written"
echo "composed: a one-byte record change is refused before packing"

for needle in \
  "$GAMMA_EVALUATOR_TAPE_SHA256" "$DELTA_COMPILER_PACKED_SHA256" \
  "$DELTA_COMPILER_PACKED_SIZE"
do
  grep -q "$needle" "$OMEGA_PATH_DELTA_COMPILER_COMPOSED" ||
    fail "delta_compiler.composed lacks bound record $needle"
done
for needle in \
  "$DELTA_COMPILER_ENTRY_SHA256" "$DELTA_COMPILER_MANIFEST_SHA256" \
  "$DELTA_COMPILER_COMPOSED_SHA256" "$DELTA_COMPILER_PACKED_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/3_delta/README.md" ||
    fail "bootstrap/3_delta/README.md lacks bound record $needle"
done
for needle in "$DELTA_COMPILER_PACKED_SIZE" "$DELTA_COMPILER_PACKED_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/delta/normalization/compiler.tsv" ||
    fail "normalization compiler.tsv lacks bound record $needle"
done
grep -q "$GAMMA_EVALUATOR_TAPE_SHA256" \
  "$OMEGA_REPO_ROOT/bootstrap/2_gamma/EVALUATOR_PROFILE.md" ||
  fail "EVALUATOR_PROFILE.md lacks bound evaluator identity"
echo "records: bound identities match delta_compiler.composed, README.md, compiler.tsv, and EVALUATOR_PROFILE.md"

echo "Delta identity: bound closure materialized exactly; corrupted entry, manifest, member, and record refused"
