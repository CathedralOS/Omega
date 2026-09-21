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

cp "$SEED" "$TMP/appended-seed"
printf 'tail' >> "$TMP/appended-seed"
rc=0
stamp_seed "$TMP/probe.tape" "$TMP/appended-seed" "$TMP/refused-appended" \
  2>"$TMP/appended.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "appended seed: expected exit 3, got $rc"
grep -q 'bootstrap/0_alpha/README.md' "$TMP/appended.err" ||
  fail "appended seed: refusal did not cite the retention inventory"
[ ! -e "$TMP/refused-appended" ] ||
  fail "appended seed: destination was written"
echo "append: a container with trailing bytes is refused before stamping"

# The legs above damage only the host-selected seed. Stamping must also refuse
# a different audited container, and each container's own bound record must
# refuse corrupt, truncated, and appended copies — a valid container for
# another host is not this host's seed.
for other in alpha_arm64_macos alpha_x64_linux alpha_x64_windows.exe; do
  [ "$other" = "$ALPHA_SEED" ] && continue
  case "$other" in
    alpha_arm64_macos)
      OTHER_SIZE=$ALPHA_SEED_ARM64_MACOS_SIZE
      OTHER_SHA256=$ALPHA_SEED_ARM64_MACOS_SHA256 ;;
    alpha_x64_linux)
      OTHER_SIZE=$ALPHA_SEED_X64_LINUX_SIZE
      OTHER_SHA256=$ALPHA_SEED_X64_LINUX_SHA256 ;;
    *)
      OTHER_SIZE=$ALPHA_SEED_X64_WINDOWS_SIZE
      OTHER_SHA256=$ALPHA_SEED_X64_WINDOWS_SHA256 ;;
  esac
  OTHER_SEED="$OMEGA_PATH_ALPHA/$other"

  rc=0
  stamp_seed "$TMP/probe.tape" "$OTHER_SEED" "$TMP/refused-$other" \
    2>"$TMP/substitute-$other.err" || rc=$?
  [ "$rc" = 3 ] ||
    fail "substituted seed $other: expected exit 3, got $rc"
  grep -q 'bootstrap/0_alpha/README.md' "$TMP/substitute-$other.err" ||
    fail "substituted seed $other: refusal did not cite the retention inventory"
  [ ! -e "$TMP/refused-$other" ] ||
    fail "substituted seed $other: destination was written"
  echo "substitute: the audited $other container is refused as this host's seed"

  cp "$OTHER_SEED" "$TMP/corrupt-$other"
  if [ "$(od -An -tx1 -j 100 -N1 "$TMP/corrupt-$other" | tr -d ' ')" = "ff" ]; then
    printf '\000' | dd of="$TMP/corrupt-$other" bs=1 seek=100 conv=notrunc status=none
  else
    printf '\377' | dd of="$TMP/corrupt-$other" bs=1 seek=100 conv=notrunc status=none
  fi
  rc=0
  require_bound_identity "$other" "$TMP/corrupt-$other" \
    "$OTHER_SIZE" "$OTHER_SHA256" "bootstrap/0_alpha/README.md" \
    2>"$TMP/corrupt-$other.err" || rc=$?
  [ "$rc" = 3 ] ||
    fail "corrupted $other: expected exit 3, got $rc"
  grep -q "$OTHER_SHA256" "$TMP/corrupt-$other.err" ||
    fail "corrupted $other: refusal did not name the audited digest"
  echo "corrupt: a one-byte $other change fails its bound identity"

  head -c $((OTHER_SIZE - 1)) "$OTHER_SEED" > "$TMP/truncated-$other"
  rc=0
  require_bound_identity "$other" "$TMP/truncated-$other" \
    "$OTHER_SIZE" "$OTHER_SHA256" "bootstrap/0_alpha/README.md" \
    2>/dev/null || rc=$?
  [ "$rc" = 3 ] ||
    fail "truncated $other: expected exit 3, got $rc"
  echo "truncate: a truncated $other fails its bound identity"

  cp "$OTHER_SEED" "$TMP/appended-$other"
  printf 'tail' >> "$TMP/appended-$other"
  rc=0
  require_bound_identity "$other" "$TMP/appended-$other" \
    "$OTHER_SIZE" "$OTHER_SHA256" "bootstrap/0_alpha/README.md" \
    2>/dev/null || rc=$?
  [ "$rc" = 3 ] ||
    fail "appended $other: expected exit 3, got $rc"
  echo "append: trailing bytes on $other fail its bound identity"
done

for needle in \
  "$ALPHA_SEED_ARM64_MACOS_SHA256" "$ALPHA_SEED_X64_WINDOWS_SHA256" \
  "$ALPHA_SEED_X64_LINUX_SHA256" \
  "16,942,368" "16,782,336" "16,789,856"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/0_alpha/README.md" ||
    fail "README retention inventory lacks bound record $needle"
done
echo "records: bound identities match bootstrap/0_alpha/README.md"

echo "Alpha identity: audited seed stamped exactly; corrupted, truncated, appended, and substituted containers refused"
