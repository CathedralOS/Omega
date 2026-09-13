#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "Epsilon identity: skipped (python3 absent)"
  exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

require_epsilon_evaluator_identity ||
  fail "bound identity check refused the canonical checkout"
echo "canonical: bound manifest and packed closure pass"

materialize_epsilon_evaluator "$TMP/evaluator.delta" ||
  fail "materialization of the bound closure failed"
require_bound_identity "materialized evaluator" "$TMP/evaluator.delta" \
  "$EPSILON_EVALUATOR_PACKED_SIZE" "$EPSILON_EVALUATOR_PACKED_SHA256" \
  "bootstrap/4_epsilon/README.md" ||
  fail "materialized closure differs from the bound identity"
echo "materialize: packed evaluator is exactly the bound member bytes"

cp -R "$OMEGA_PATH_EPSILON" "$TMP/4_epsilon"
head -c $((EPSILON_EVALUATOR_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" > "$TMP/4_epsilon/epsilon_compiler.delta.sources"
rc=0
(
  export OMEGA_PATH_EPSILON_COMPILER_SOURCES=$TMP/4_epsilon/epsilon_compiler.delta.sources
  materialize_epsilon_evaluator "$TMP/refused-truncated"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated manifest: expected exit 3, got $rc"
[ ! -e "$TMP/refused-truncated" ] ||
  fail "truncated manifest: destination was written"
echo "truncate: a truncated manifest is refused before packing"

cp "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$TMP/4_epsilon/epsilon_compiler.delta.sources"
CORRUPT_MEMBER="$TMP/4_epsilon/lexical/tokens.delta"
if [ "$(od -An -tc -j 100 -N1 "$CORRUPT_MEMBER" | tr -d ' ')" = "a" ]; then
  printf 'b' | dd of="$CORRUPT_MEMBER" bs=1 seek=100 conv=notrunc status=none
else
  printf 'a' | dd of="$CORRUPT_MEMBER" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_EPSILON_COMPILER_SOURCES=$TMP/4_epsilon/epsilon_compiler.delta.sources
  materialize_epsilon_evaluator "$TMP/refused-member"
) 2>"$TMP/corrupt-member.err" || rc=$?
[ "$rc" != 0 ] ||
  fail "corrupted member: materialization unexpectedly succeeded"
grep -q 'member digest changed' "$TMP/corrupt-member.err" ||
  fail "corrupted member: refusal did not name the member digest"
[ ! -e "$TMP/refused-member" ] ||
  fail "corrupted member: destination was written"
echo "member: a one-byte member change is refused during packing"

for needle in \
  "$EPSILON_EVALUATOR_MANIFEST_SHA256" "$EPSILON_EVALUATOR_PACKED_SHA256" \
  "617,354"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/4_epsilon/README.md" ||
    fail "bootstrap/4_epsilon/README.md lacks bound record $needle"
done
for needle in "$EPSILON_EVALUATOR_PACKED_SIZE" "$EPSILON_EVALUATOR_PACKED_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/epsilon/checking/run.sh" ||
    fail "epsilon checking gate lacks bound record $needle"
done
echo "records: bound identities match bootstrap/4_epsilon/README.md and the checking gate"

echo "Epsilon identity: bound closure materialized exactly; corrupted manifest and member refused"
