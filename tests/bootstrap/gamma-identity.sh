#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

require_gamma_evaluator_identity ||
  fail "bound identity check refused the canonical checkout"
echo "canonical: selected source and tape pass the bound identity check"

materialize_gamma_evaluator "$TMP/evaluator" ||
  fail "materialization of the selected tape failed"
{ printf '\177\041\000\000'; cat "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE"; } > "$TMP/expected"
tape_in_seed "$TMP/evaluator" > "$TMP/stamped"
cmp -s "$TMP/stamped" "$TMP/expected" ||
  fail "stamped [len][tape] payload differs from the selected tape"
echo "materialize: stamped seed embeds exactly [len][selected tape]"

cp "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" "$TMP/corrupt.tape"
if [ "$(od -An -tx1 -j 100 -N1 "$TMP/corrupt.tape" | tr -d ' ')" = "ff" ]; then
  printf '\000' | dd of="$TMP/corrupt.tape" bs=1 seek=100 conv=notrunc status=none
else
  printf '\377' | dd of="$TMP/corrupt.tape" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_GAMMA_EVALUATOR_TAPE=$TMP/corrupt.tape
  . "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
  materialize_gamma_evaluator "$TMP/refused"
) 2>"$TMP/corrupt.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted tape: expected exit 3, got $rc"
grep -q 'EVALUATOR_PROFILE.md' "$TMP/corrupt.err" ||
  fail "corrupted tape: refusal did not cite EVALUATOR_PROFILE.md"
[ ! -e "$TMP/refused" ] ||
  fail "corrupted tape: destination was written"
echo "corrupt: a one-byte tape change is refused before stamping"

head -c 8574 "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" > "$TMP/truncated.tape"
rc=0
(
  export OMEGA_PATH_GAMMA_EVALUATOR_TAPE=$TMP/truncated.tape
  . "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
  materialize_gamma_evaluator "$TMP/refused-truncated"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated tape: expected exit 3, got $rc"
[ ! -e "$TMP/refused-truncated" ] ||
  fail "truncated tape: destination was written"
echo "truncate: a truncated tape is refused before stamping"

for needle in \
  "$GAMMA_EVALUATOR_SOURCE_SHA256" "$GAMMA_EVALUATOR_TAPE_SHA256" "47,748" "8,575"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/2_gamma/EVALUATOR_PROFILE.md" ||
    fail "EVALUATOR_PROFILE.md lacks bound record $needle"
done
for needle in \
  "$GAMMA_EVALUATOR_SOURCE_SIZE" "$GAMMA_EVALUATOR_SOURCE_SHA256" \
  "$GAMMA_EVALUATOR_TAPE_SIZE" "$GAMMA_EVALUATOR_TAPE_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/gamma/heap-boundary/evaluator.tsv" ||
    fail "evaluator.tsv lacks bound record $needle"
done
grep -q "$GAMMA_EVALUATOR_TAPE_SHA256" \
  "$OMEGA_REPO_ROOT/bootstrap/3_delta/delta_compiler.composed" ||
  fail "delta_compiler.composed lacks bound evaluator identity"
echo "records: bound identities match EVALUATOR_PROFILE.md, evaluator.tsv, and delta_compiler.composed"

echo "Gamma identity: selected evaluator stamped exactly; corrupted and truncated tapes refused"
