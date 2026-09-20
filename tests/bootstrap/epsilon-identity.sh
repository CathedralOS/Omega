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

require_epsilon_execution_driver_identity ||
  fail "bound driver identity check refused the canonical driver"
echo "driver: canonical execution driver passes the bound identity check"

cp "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER" "$TMP/corrupt-driver.delta"
if [ "$(od -An -tc -j 100 -N1 "$TMP/corrupt-driver.delta" | tr -d ' ')" = "a" ]; then
  printf 'b' | dd of="$TMP/corrupt-driver.delta" bs=1 seek=100 conv=notrunc status=none
else
  printf 'a' | dd of="$TMP/corrupt-driver.delta" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_EPSILON_EXECUTION_DRIVER=$TMP/corrupt-driver.delta
  require_epsilon_execution_driver_identity
) 2>"$TMP/corrupt-driver.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted driver: expected exit 3, got $rc"
grep -q 'interpreted-omega-experiment/README.md' "$TMP/corrupt-driver.err" ||
  fail "corrupted driver: refusal did not cite the driver record"
echo "driver: a one-byte driver change is refused"

head -c $((EPSILON_EXECUTION_DRIVER_SIZE - 1)) \
  "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER" > "$TMP/truncated-driver.delta"
rc=0
(
  export OMEGA_PATH_EPSILON_EXECUTION_DRIVER=$TMP/truncated-driver.delta
  require_epsilon_execution_driver_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated driver: expected exit 3, got $rc"
echo "driver: a truncated driver is refused"

require_epsilon_evaluator_entry_identity ||
  fail "bound entry identity check refused the canonical entry"
echo "entry: canonical evaluator entry passes the bound identity check"

EPSILON_ENTRY_FILE="$OMEGA_REPO_ROOT/tests/epsilon/evaluator-entry/evaluator_entry.delta"
cp "$EPSILON_ENTRY_FILE" "$TMP/corrupt-entry.delta"
if [ "$(od -An -tc -j 100 -N1 "$TMP/corrupt-entry.delta" | tr -d ' ')" = "a" ]; then
  printf 'b' | dd of="$TMP/corrupt-entry.delta" bs=1 seek=100 conv=notrunc status=none
else
  printf 'a' | dd of="$TMP/corrupt-entry.delta" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_EPSILON_EVALUATOR_ENTRY=$TMP/corrupt-entry.delta
  require_epsilon_evaluator_entry_identity
) 2>"$TMP/corrupt-entry.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted entry: expected exit 3, got $rc"
grep -q 'evaluator-entry/README.md' "$TMP/corrupt-entry.err" ||
  fail "corrupted entry: refusal did not cite the entry record"
echo "entry: a one-byte entry change is refused"

head -c $((EPSILON_EVALUATOR_ENTRY_SIZE - 1)) \
  "$EPSILON_ENTRY_FILE" > "$TMP/truncated-entry.delta"
rc=0
(
  export OMEGA_PATH_EPSILON_EVALUATOR_ENTRY=$TMP/truncated-entry.delta
  require_epsilon_evaluator_entry_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated entry: expected exit 3, got $rc"
echo "entry: a truncated entry is refused"

printf 'not the reconstructed canonical receipt' > "$TMP/short-entry-receipt"
rc=0
require_epsilon_evaluator_entry_receipt_identity "$TMP/short-entry-receipt" \
  2>"$TMP/short-entry-receipt.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "short canonical receipt: expected exit 3, got $rc"
grep -q 'evaluator-entry/README.md' "$TMP/short-entry-receipt.err" ||
  fail "short canonical receipt: refusal did not cite the receipt record"
echo "entry receipt: a wrong-size reconstruction is refused"

head -c "$EPSILON_EVALUATOR_ENTRY_RECEIPT_SIZE" /dev/zero > "$TMP/zero-entry-receipt"
rc=0
require_epsilon_evaluator_entry_receipt_identity "$TMP/zero-entry-receipt" \
  2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "zero canonical receipt: expected exit 3, got $rc"
echo "entry receipt: a same-size divergent reconstruction is refused"

printf 'not the reconstructed evaluator receipt' > "$TMP/short-receipt"
rc=0
require_epsilon_evaluator_receipt_identity "$TMP/short-receipt" \
  2>"$TMP/short-receipt.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "short receipt: expected exit 3, got $rc"
grep -q 'interpreted-omega-experiment/README.md' "$TMP/short-receipt.err" ||
  fail "short receipt: refusal did not cite the receipt record"
echo "receipt: a wrong-size reconstruction is refused"

head -c "$EPSILON_EVALUATOR_RECEIPT_SIZE" /dev/zero > "$TMP/zero-receipt"
rc=0
require_epsilon_evaluator_receipt_identity "$TMP/zero-receipt" \
  2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "zero receipt: expected exit 3, got $rc"
echo "receipt: a same-size divergent reconstruction is refused"

# Gate-local driver entries packed on top of the bound member closure:
# single-file drivers bind the file directly; controls closures bind the
# gate-owned manifest plus the packed bytes the manifest repacks to.
require_epsilon_checking_driver_identity ||
  fail "bound checking driver check refused the canonical driver"
require_epsilon_array_storage_driver_identity ||
  fail "bound array-storage driver check refused the canonical driver"
echo "drivers: bound checking and array-storage drivers pass the identity check"

cp "$OMEGA_PATH_EPSILON_CHECKING_DRIVER" "$TMP/corrupt-checking-driver.delta"
printf 'x' | dd of="$TMP/corrupt-checking-driver.delta" bs=1 seek=0 conv=notrunc status=none
rc=0
(
  export OMEGA_PATH_EPSILON_CHECKING_DRIVER=$TMP/corrupt-checking-driver.delta
  require_epsilon_checking_driver_identity
) 2>"$TMP/corrupt-checking-driver.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted checking driver: expected exit 3, got $rc"
grep -q 'checking/README.md' "$TMP/corrupt-checking-driver.err" ||
  fail "corrupted checking driver: refusal did not cite the driver record"
head -c $((EPSILON_CHECKING_DRIVER_SIZE - 1)) \
  "$OMEGA_PATH_EPSILON_CHECKING_DRIVER" > "$TMP/truncated-checking-driver.delta"
rc=0
(
  export OMEGA_PATH_EPSILON_CHECKING_DRIVER=$TMP/truncated-checking-driver.delta
  require_epsilon_checking_driver_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated checking driver: expected exit 3, got $rc"
echo "drivers: a corrupted or truncated checking driver is refused"

cp "$OMEGA_PATH_EPSILON_ARRAY_STORAGE_DRIVER" "$TMP/corrupt-array-driver.delta"
printf 'x' | dd of="$TMP/corrupt-array-driver.delta" bs=1 seek=0 conv=notrunc status=none
rc=0
(
  export OMEGA_PATH_EPSILON_ARRAY_STORAGE_DRIVER=$TMP/corrupt-array-driver.delta
  require_epsilon_array_storage_driver_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted array-storage driver: expected exit 3, got $rc"
head -c $((EPSILON_ARRAY_STORAGE_DRIVER_SIZE - 1)) \
  "$OMEGA_PATH_EPSILON_ARRAY_STORAGE_DRIVER" > "$TMP/truncated-array-driver.delta"
rc=0
(
  export OMEGA_PATH_EPSILON_ARRAY_STORAGE_DRIVER=$TMP/truncated-array-driver.delta
  require_epsilon_array_storage_driver_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated array-storage driver: expected exit 3, got $rc"
echo "drivers: a corrupted or truncated array-storage driver is refused"

require_epsilon_checking_invariants_controls_identity ||
  fail "bound checking-invariants controls check refused the canonical closure"
require_epsilon_runtime_invariants_controls_identity ||
  fail "bound runtime-invariants controls check refused the canonical closure"
require_epsilon_runtime_references_controls_identity ||
  fail "bound runtime-references controls check refused the canonical closure"
require_epsilon_source_views_controls_identity ||
  fail "bound source-views controls check refused the canonical closure"
echo "controls: bound gate-owned closures repack exactly"

cp -R "$OMEGA_REPO_ROOT/tests/epsilon/checking-invariants" "$TMP/checking-invariants"
head -c $((EPSILON_CHECKING_INVARIANTS_CONTROLS_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_EPSILON_CHECKING_INVARIANTS_CONTROLS_SOURCES" \
  > "$TMP/checking-invariants/checking_invariants.delta.sources"
rc=0
(
  export OMEGA_PATH_EPSILON_CHECKING_INVARIANTS_CONTROLS_SOURCES=$TMP/checking-invariants/checking_invariants.delta.sources
  require_epsilon_checking_invariants_controls_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated checking-invariants manifest: expected exit 3, got $rc"
cp "$OMEGA_PATH_EPSILON_CHECKING_INVARIANTS_CONTROLS_SOURCES" \
  "$TMP/checking-invariants/checking_invariants.delta.sources"
if [ "$(od -An -tc -j 100 -N1 "$TMP/checking-invariants/observations.delta" | tr -d ' ')" = "a" ]; then
  printf 'b' | dd of="$TMP/checking-invariants/observations.delta" bs=1 seek=100 conv=notrunc status=none
else
  printf 'a' | dd of="$TMP/checking-invariants/observations.delta" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_EPSILON_CHECKING_INVARIANTS_CONTROLS_SOURCES=$TMP/checking-invariants/checking_invariants.delta.sources
  require_epsilon_checking_invariants_controls_identity
) 2>"$TMP/corrupt-ci-member.err" || rc=$?
[ "$rc" != 0 ] ||
  fail "corrupted checking-invariants member: repack unexpectedly succeeded"
grep -q 'digest' "$TMP/corrupt-ci-member.err" ||
  fail "corrupted checking-invariants member: refusal did not name the member digest"
echo "controls: a truncated manifest or changed checking-invariants member refuses"

cp -R "$OMEGA_REPO_ROOT/tests/epsilon/runtime-invariants" "$TMP/runtime-invariants"
head -c $((EPSILON_RUNTIME_INVARIANTS_CONTROLS_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_EPSILON_RUNTIME_INVARIANTS_CONTROLS_SOURCES" \
  > "$TMP/runtime-invariants/runtime_invariants.delta.sources"
rc=0
(
  export OMEGA_PATH_EPSILON_RUNTIME_INVARIANTS_CONTROLS_SOURCES=$TMP/runtime-invariants/runtime_invariants.delta.sources
  require_epsilon_runtime_invariants_controls_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated runtime-invariants manifest: expected exit 3, got $rc"

cp -R "$OMEGA_REPO_ROOT/tests/epsilon/runtime-references" "$TMP/runtime-references"
head -c $((EPSILON_RUNTIME_REFERENCES_CONTROLS_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_EPSILON_RUNTIME_REFERENCES_CONTROLS_SOURCES" \
  > "$TMP/runtime-references/runtime_references.delta.sources"
rc=0
(
  export OMEGA_PATH_EPSILON_RUNTIME_REFERENCES_CONTROLS_SOURCES=$TMP/runtime-references/runtime_references.delta.sources
  require_epsilon_runtime_references_controls_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated runtime-references manifest: expected exit 3, got $rc"

cp -R "$OMEGA_REPO_ROOT/tests/epsilon/source-views" "$TMP/source-views"
head -c $((EPSILON_SOURCE_VIEWS_CONTROLS_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_EPSILON_SOURCE_VIEWS_CONTROLS_SOURCES" \
  > "$TMP/source-views/controls/source_views.delta.sources"
rc=0
(
  export OMEGA_PATH_EPSILON_SOURCE_VIEWS_CONTROLS_SOURCES=$TMP/source-views/controls/source_views.delta.sources
  require_epsilon_source_views_controls_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated source-views manifest: expected exit 3, got $rc"
echo "controls: a truncated manifest refuses for each remaining controls closure"

for needle in "$EPSILON_CHECKING_DRIVER_SIZE" "$EPSILON_CHECKING_DRIVER_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/epsilon/checking/run.sh" ||
    fail "epsilon checking gate lacks bound driver record $needle"
done
for needle in "$EPSILON_ARRAY_STORAGE_DRIVER_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/epsilon/array-storage/gate.py" ||
    fail "epsilon array-storage gate lacks bound driver record $needle"
done
echo "records: bound gate-local driver pins match the consuming gates"

for needle in \
  "$EPSILON_EVALUATOR_MANIFEST_SHA256" "$EPSILON_EVALUATOR_PACKED_SHA256" \
  "$EPSILON_EXECUTION_DRIVER_SHA256" "$EPSILON_EVALUATOR_RECEIPT_SHA256" \
  "15,163" "617,354"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/4_epsilon/README.md" ||
    fail "bootstrap/4_epsilon/README.md lacks bound record $needle"
done
for needle in \
  "$EPSILON_EXECUTION_DRIVER_SHA256" "$EPSILON_EVALUATOR_RECEIPT_SHA256" \
  "617,354" "2,565" "721,484"
do
  grep -q "$needle" \
    "$OMEGA_REPO_ROOT/tests/epsilon/interpreted-omega-experiment/README.md" ||
    fail "interpreted-omega-experiment README lacks bound record $needle"
done
for needle in "$EPSILON_EVALUATOR_PACKED_SIZE" "$EPSILON_EVALUATOR_PACKED_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/epsilon/checking/run.sh" ||
    fail "epsilon checking gate lacks bound record $needle"
done
for record in \
  tests/bootstrap/omega-parser/gate.py \
  tests/bootstrap/omega-outcome/gate.py \
  tests/bootstrap/source-closure.py \
  tests/epsilon/array-storage/gate.py \
  tests/epsilon/interpreted-omega-experiment/run.sh
do
  for needle in \
    "$EPSILON_EVALUATOR_PACKED_SIZE" "$EPSILON_EVALUATOR_PACKED_SHA256"
  do
    grep -q "$needle" "$OMEGA_REPO_ROOT/$record" ||
      fail "$record lacks bound evaluator-closure record $needle"
  done
done
for gate in \
  tests/bootstrap/omega-parser/gate.py \
  tests/bootstrap/omega-outcome/gate.py \
  tests/bootstrap/omega-executable/gate.py \
  tests/epsilon/array-storage/gate.py \
  tests/epsilon/interpreted-omega-experiment/run.sh
do
  for needle in \
    "$EPSILON_EXECUTION_DRIVER_SHA256" "$EPSILON_EVALUATOR_RECEIPT_SIZE" \
    "$EPSILON_EVALUATOR_RECEIPT_SHA256"
  do
    grep -q "$needle" "$OMEGA_REPO_ROOT/$gate" ||
      fail "$gate lacks bound record $needle"
  done
done
for needle in \
  "$EPSILON_EVALUATOR_PACKED_SHA256" "$EPSILON_EXECUTION_DRIVER_SHA256" \
  "$EPSILON_EVALUATOR_RECEIPT_SHA256" "617,354" "2,565" "721,484"
do
  grep -q "$needle" \
    "$OMEGA_REPO_ROOT/bootstrap/4_epsilon/EVALUATOR_PROFILE.md" ||
    fail "4_epsilon EVALUATOR_PROFILE.md lacks bound record $needle"
done
for needle in \
  "$EPSILON_EVALUATOR_ENTRY_SHA256" \
  "$EPSILON_EVALUATOR_ENTRY_RECEIPT_SHA256" \
  "$EPSILON_EVALUATOR_PACKED_SHA256" "10,950" "729,060"
do
  grep -q "$needle" \
    "$OMEGA_REPO_ROOT/bootstrap/4_epsilon/EVALUATOR_ENTRY.md" ||
    fail "4_epsilon EVALUATOR_ENTRY.md lacks bound record $needle"
  grep -q "$needle" \
    "$OMEGA_REPO_ROOT/tests/epsilon/evaluator-entry/README.md" ||
    fail "evaluator-entry README lacks bound record $needle"
done
echo "records: bound identities match the rung README, the edge profile, the entry envelope, the driver and entry owner READMEs, and every consuming gate"

echo "Epsilon identity: bound closure materialized exactly; corrupted manifest, member, driver, entry, receipts, gate-local drivers, and controls refused"
