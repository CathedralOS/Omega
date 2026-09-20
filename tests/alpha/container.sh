#!/usr/bin/env sh
# Validates the audited Alpha seed containers as native executables, on every
# host — including ones that cannot execute either seed:
#
#   identity   - BOTH committed containers match the retention inventory
#                records (size + SHA-256). alpha-identity.sh binds only the
#                host-selected seed; the other container's bytes were never
#                verified on non-native hosts before this gate.
#   structure  - each container parses as its native format (PE32+ x86-64,
#                arm64 Mach-O), the entry point lands in executable code,
#                required loader imports/signature exist, and the recorded
#                hole offset is exactly the tape section's raw extent — the
#                precondition stamp_seed relies on but does not check.
#   stamping   - a stamped artifact still satisfies the native contract: the
#                hole carries exactly [length][tape] with a zeroed tail and
#                every byte outside the hole unchanged. The host-selected seed
#                goes through the real stamper; the other container is stamped
#                at its own recorded hole offset, since only its native host's
#                profile addresses it (and macOS additionally re-signs there).
#
# Runs wherever Python 3 does; it inspects bytes, it does not exec them.
# Executable conformance remains on the native hosts (conformance.sh).
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

if ! command -v python3 >/dev/null 2>&1; then
  echo "Alpha container SKIP - python3 not found (native structure check needs it)"
  exit 0
fi

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

WIN_SEED="$OMEGA_PATH_ALPHA/alpha_x64_windows.exe"
MAC_SEED="$OMEGA_PATH_ALPHA/alpha_arm64_macos"
CONTAINER_PY="$TEST_DIR/container.py"
INVENTORY="bootstrap/0_alpha/README.md"

echo "--- container identity (both seeds, every host) ---"
require_bound_identity "alpha_x64_windows.exe" "$WIN_SEED" \
  "$ALPHA_SEED_X64_WINDOWS_SIZE" "$ALPHA_SEED_X64_WINDOWS_SHA256" \
  "$INVENTORY" || fail "Windows container fails the bound identity"
require_bound_identity "alpha_arm64_macos" "$MAC_SEED" \
  "$ALPHA_SEED_ARM64_MACOS_SIZE" "$ALPHA_SEED_ARM64_MACOS_SHA256" \
  "$INVENTORY" || fail "macOS container fails the bound identity"
echo "identity ✓ — both committed containers match the retention inventory"

echo "--- container structure (native format + hole contract) ---"
python3 "$CONTAINER_PY" "$WIN_SEED" --format pe \
  --hole-off "$ALPHA_SEED_X64_WINDOWS_HOLE_OFF" \
  --hole-size "$ALPHA_SEED_HOLE_SIZE" \
  --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" || fail "Windows container structure"
python3 "$CONTAINER_PY" "$MAC_SEED" --format macho \
  --hole-off "$ALPHA_SEED_ARM64_MACOS_HOLE_OFF" \
  --hole-size "$ALPHA_SEED_HOLE_SIZE" \
  --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" || fail "macOS container structure"

echo "--- stamped artifacts keep the native contract ---"
printf 'probe-tape!' > "$TMP/probe.tape"
stamp_seed "$TMP/probe.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/stamped" ||
  fail "stamping the selected seed failed"
case "$ALPHA_SEED" in
  alpha_x64_windows.exe)
    STAMP_FORMAT=pe
    STAMP_PRISTINE="$WIN_SEED"
    OTHER_SEED=$MAC_SEED
    OTHER_FORMAT=macho
    OTHER_OFF=$ALPHA_SEED_ARM64_MACOS_HOLE_OFF
    ;;
  *)
    STAMP_FORMAT=macho
    # Darwin re-signs after stamping, which rewrites bytes outside the hole;
    # the byte-exact outside-hole comparison applies only where no re-sign ran.
    STAMP_PRISTINE=
    OTHER_SEED=$WIN_SEED
    OTHER_FORMAT=pe
    OTHER_OFF=$ALPHA_SEED_X64_WINDOWS_HOLE_OFF
    ;;
esac
if [ -n "$STAMP_PRISTINE" ]; then
  python3 "$CONTAINER_PY" "$TMP/stamped" --format "$STAMP_FORMAT" \
    --hole-off "$HOLE_OFF" --hole-size "$ALPHA_SEED_HOLE_SIZE" \
    --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" --stamped --tape "$TMP/probe.tape" \
    --pristine "$STAMP_PRISTINE" || fail "stamped artifact structure"
else
  python3 "$CONTAINER_PY" "$TMP/stamped" --format "$STAMP_FORMAT" \
    --hole-off "$HOLE_OFF" --hole-size "$ALPHA_SEED_HOLE_SIZE" \
    --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" --stamped --tape "$TMP/probe.tape" ||
    fail "stamped artifact structure"
fi

cp "$OTHER_SEED" "$TMP/other-stamped"
L=$(wc -c < "$TMP/probe.tape" | tr -d ' ')
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) \
$(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
  | dd of="$TMP/other-stamped" bs=1 seek="$OTHER_OFF" conv=notrunc status=none
dd if="$TMP/probe.tape" of="$TMP/other-stamped" bs=1 \
  seek=$((OTHER_OFF + 4)) conv=notrunc status=none
python3 "$CONTAINER_PY" "$TMP/other-stamped" --format "$OTHER_FORMAT" \
  --hole-off "$OTHER_OFF" --hole-size "$ALPHA_SEED_HOLE_SIZE" \
  --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" --stamped --tape "$TMP/probe.tape" \
  --pristine "$OTHER_SEED" || fail "non-host container stamp contract"

echo "Alpha container: both audited seeds structurally valid; the stamping hole matches the tape section on every host"
