#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"

require_alpha_seed_identity "$SEED" ||
  fail "bound identity check refused the canonical seed"
echo "canonical: audited seed passes the bound identity check"

printf 'probe-tape!' > "$TMP/probe.tape"
stamp_seed "$TMP/probe.tape" "$SEED" "$TMP/stamped.exe" ||
  fail "stamping the audited seed failed"
{ printf '\013\000\000\000'; cat "$TMP/probe.tape"; } > "$TMP/expected"
tape_in_seed "$TMP/stamped.exe" > "$TMP/stamped"
cmp -s "$TMP/stamped" "$TMP/expected" ||
  fail "stamped [len][tape] payload differs from the probe tape"
echo "materialize: stamped seed embeds exactly [len][probe tape]"

cp "$SEED" "$TMP/corrupt-seed"
if [ "$(od -An -tx1 -j 100 -N1 "$TMP/corrupt-seed" | tr -d ' ')" = "ff" ]; then
  printf '\000' | dd of="$TMP/corrupt-seed" bs=1 seek=100 conv=notrunc status=none
else
  printf '\377' | dd of="$TMP/corrupt-seed" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
stamp_seed "$TMP/probe.tape" "$TMP/corrupt-seed" "$TMP/refused" \
  2>"$TMP/corrupt.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted seed: expected exit 3, got $rc"
grep -q 'bootstrap/0_alpha/README.md' "$TMP/corrupt.err" ||
  fail "corrupted seed: refusal did not cite the retention inventory"
[ ! -e "$TMP/refused" ] ||
  fail "corrupted seed: destination was written"
echo "corrupt: a one-byte container change is refused before stamping"

TRUNCATED_SIZE=$((ALPHA_SEED_SIZE - 1))
head -c "$TRUNCATED_SIZE" "$SEED" > "$TMP/truncated-seed"
rc=0
stamp_seed "$TMP/probe.tape" "$TMP/truncated-seed" "$TMP/refused-truncated" \
  2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated seed: expected exit 3, got $rc"
[ ! -e "$TMP/refused-truncated" ] ||
  fail "truncated seed: destination was written"
echo "truncate: a truncated container is refused before stamping"

for needle in \
  "$ALPHA_SEED_ARM64_MACOS_SHA256" "$ALPHA_SEED_X64_WINDOWS_SHA256" \
  "16,942,384" "16,782,336"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/0_alpha/README.md" ||
    fail "README retention inventory lacks bound record $needle"
done
echo "records: bound identities match bootstrap/0_alpha/README.md"

echo "Alpha identity: audited seed stamped exactly; corrupted and truncated containers refused"
