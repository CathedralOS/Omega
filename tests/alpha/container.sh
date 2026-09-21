#!/usr/bin/env sh
# Validates the audited Alpha seed containers as native executables, on every
# host — including ones that cannot execute them:
#
#   identity   - ALL committed containers match the retention inventory
#                records (size + SHA-256). alpha-identity.sh binds only the
#                host-selected seed; the other containers' bytes were never
#                verified on non-native hosts before this gate.
#   structure  - each container parses as its native format (PE32+ x86-64,
#                arm64 Mach-O, static x86-64 ELF64), the entry point lands in
#                executable code,
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
LIN_SEED="$OMEGA_PATH_ALPHA/alpha_x64_linux"
CONTAINER_PY="$TEST_DIR/container.py"
INVENTORY="bootstrap/0_alpha/README.md"

# The Linux seed's bound identity lives in the retention inventory until
# seed_env.sh carries the third container's constants (host-selection wiring
# belongs to the gate-enablement item, not this artifact leg).
LIN_ROW=$(grep -F '`alpha_x64_linux`' "$OMEGA_REPO_ROOT/$INVENTORY") ||
  fail "retention inventory lacks an alpha_x64_linux row"
LIN_SIZE=$(printf '%s\n' "$LIN_ROW" | sed -n 's/.*| *\([0-9,][0-9,]*\) *|.*/\1/p' | tr -d ',')
LIN_SHA=$(printf '%s\n' "$LIN_ROW" | sed -n 's/.*| *`\([0-9a-f]\{64\}\)` *|.*/\1/p')
LIN_HOLE_OFF=12288
[ -n "$LIN_SIZE" ] && [ -n "$LIN_SHA" ] ||
  fail "alpha_x64_linux inventory row is not parseable as '| name | bytes | sha256 |'"

echo "--- container identity (all seeds, every host) ---"
require_bound_identity "alpha_x64_windows.exe" "$WIN_SEED" \
  "$ALPHA_SEED_X64_WINDOWS_SIZE" "$ALPHA_SEED_X64_WINDOWS_SHA256" \
  "$INVENTORY" || fail "Windows container fails the bound identity"
require_bound_identity "alpha_arm64_macos" "$MAC_SEED" \
  "$ALPHA_SEED_ARM64_MACOS_SIZE" "$ALPHA_SEED_ARM64_MACOS_SHA256" \
  "$INVENTORY" || fail "macOS container fails the bound identity"
require_bound_identity "alpha_x64_linux" "$LIN_SEED" \
  "$LIN_SIZE" "$LIN_SHA" \
  "$INVENTORY" || fail "Linux container fails the bound identity"
echo "identity ✓ — all committed containers match the retention inventory"

echo "--- container structure (native format + hole contract) ---"
python3 "$CONTAINER_PY" "$WIN_SEED" --format pe \
  --hole-off "$ALPHA_SEED_X64_WINDOWS_HOLE_OFF" \
  --hole-size "$ALPHA_SEED_HOLE_SIZE" \
  --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" || fail "Windows container structure"
python3 "$CONTAINER_PY" "$MAC_SEED" --format macho \
  --hole-off "$ALPHA_SEED_ARM64_MACOS_HOLE_OFF" \
  --hole-size "$ALPHA_SEED_HOLE_SIZE" \
  --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" || fail "macOS container structure"
python3 "$CONTAINER_PY" "$LIN_SEED" --format elf \
  --hole-off "$LIN_HOLE_OFF" \
  --hole-size "$ALPHA_SEED_HOLE_SIZE" \
  --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" || fail "Linux container structure"

echo "--- stamped artifacts keep the native contract ---"
printf 'probe-tape!' > "$TMP/probe.tape"
stamp_seed "$TMP/probe.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/stamped" ||
  fail "stamping the selected seed failed"
OTHERS=
case "$ALPHA_SEED" in
  alpha_x64_windows.exe)
    STAMP_FORMAT=pe
    STAMP_PRISTINE="$WIN_SEED"
    OTHERS="$MAC_SEED:macho:$ALPHA_SEED_ARM64_MACOS_HOLE_OFF $LIN_SEED:elf:$LIN_HOLE_OFF"
    ;;
  alpha_x64_linux)
    STAMP_FORMAT=elf
    STAMP_PRISTINE="$LIN_SEED"
    OTHERS="$WIN_SEED:pe:$ALPHA_SEED_X64_WINDOWS_HOLE_OFF $MAC_SEED:macho:$ALPHA_SEED_ARM64_MACOS_HOLE_OFF"
    ;;
  *)
    STAMP_FORMAT=macho
    # Darwin re-signs after stamping, which rewrites bytes outside the hole;
    # the byte-exact outside-hole comparison applies only where no re-sign ran.
    STAMP_PRISTINE=
    OTHERS="$WIN_SEED:pe:$ALPHA_SEED_X64_WINDOWS_HOLE_OFF $LIN_SEED:elf:$LIN_HOLE_OFF"
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

for other in $OTHERS; do
  OTHER_SEED=${other%%:*}
  rest=${other#*:}
  OTHER_FORMAT=${rest%%:*}
  OTHER_OFF=${rest#*:}
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
done

echo "Alpha container: all audited seeds structurally valid; the stamping hole matches the tape section on every host"
