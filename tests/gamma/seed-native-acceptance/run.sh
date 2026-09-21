#!/usr/bin/env sh
# Accepts the selected Gamma evaluator's seeded native containers as native
# artifacts, on every host — including ones that cannot exec either seed:
#
#   identity   - the canonical source/tape pair matches the bound evaluator
#                identities before anything is stamped.
#   acceptance - the evaluator tape is stamped into EVERY audited Alpha
#                seed: the host-selected container through the real
#                materializer (which re-signs on macOS), each non-host
#                container at its own recorded hole offset. Every stamped
#                result must satisfy the
#                native-container contract enforced by
#                tests/alpha/container.py: format structure, executable entry,
#                loader imports/signature, hole = [length][tape][zeros], and —
#                where no re-sign ran — byte-exact content outside the hole.
#   execution  - on seed-execution hosts only, the materialized container
#                runs a minimal Gamma source natively and publishes its exact
#                receipt. Other hosts exercise every host-free check above
#                and report the exec leg as skipped.
#
# The existing Alpha container gate proves the seeds' stamping contract with a
# probe tape; this gate proves it for the evaluator tape the Gamma chain
# actually ships, which is the artifact every seed-executing Gamma gate runs.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

if ! command -v python3 >/dev/null 2>&1; then
  echo "Gamma seed native acceptance: skipped (python3 not found)"
  exit 0
fi

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
CONTAINER_PY="$OMEGA_REPO_ROOT/tests/alpha/container.py"
TAPE="$OMEGA_PATH_GAMMA_EVALUATOR_TAPE"

fail() {
  echo "FAIL $1" >&2
  exit 1
}

echo "--- bound evaluator identity ---"
require_gamma_evaluator_identity ||
  fail "canonical evaluator pair differs from the bound identities"
echo "identity ok — source and tape match the selected evaluator records"

echo "--- stamped evaluator native acceptance (every audited seed) ---"
# The host-selected seed goes through the real materializer, so macOS re-signs
# and its outside-hole bytes legitimately differ from the pristine image.
materialize_gamma_evaluator "$TMP/evaluator" >/dev/null ||
  fail "materialize_gamma_evaluator refused the bound tape or seed"
LIN_SEED="$OMEGA_PATH_ALPHA/alpha_x64_linux"
WIN_SEED="$OMEGA_PATH_ALPHA/alpha_x64_windows.exe"
MAC_SEED="$OMEGA_PATH_ALPHA/alpha_arm64_macos"
case "$ALPHA_SEED" in
  alpha_x64_windows.exe)
    STAMP_FORMAT=pe
    STAMP_PRISTINE="$WIN_SEED"
    OTHERS="$MAC_SEED:macho:$ALPHA_SEED_ARM64_MACOS_HOLE_OFF $LIN_SEED:elf:$ALPHA_SEED_X64_LINUX_HOLE_OFF"
    ;;
  alpha_x64_linux)
    STAMP_FORMAT=elf
    STAMP_PRISTINE="$LIN_SEED"
    OTHERS="$WIN_SEED:pe:$ALPHA_SEED_X64_WINDOWS_HOLE_OFF $MAC_SEED:macho:$ALPHA_SEED_ARM64_MACOS_HOLE_OFF"
    ;;
  *)
    STAMP_FORMAT=macho
    STAMP_PRISTINE=
    OTHERS="$WIN_SEED:pe:$ALPHA_SEED_X64_WINDOWS_HOLE_OFF $LIN_SEED:elf:$ALPHA_SEED_X64_LINUX_HOLE_OFF"
    ;;
esac
if [ -n "$STAMP_PRISTINE" ]; then
  python3 "$CONTAINER_PY" "$TMP/evaluator" --format "$STAMP_FORMAT" \
    --hole-off "$HOLE_OFF" --hole-size "$ALPHA_SEED_HOLE_SIZE" \
    --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" --stamped --tape "$TAPE" \
    --pristine "$STAMP_PRISTINE" || fail "stamped evaluator container contract"
else
  python3 "$CONTAINER_PY" "$TMP/evaluator" --format "$STAMP_FORMAT" \
    --hole-off "$HOLE_OFF" --hole-size "$ALPHA_SEED_HOLE_SIZE" \
    --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" --stamped --tape "$TAPE" ||
    fail "stamped evaluator container contract"
fi

# Each non-host seed is stamped at its own recorded hole offset; only its
# native host's profile addresses that container (macOS re-signs there).
for other in $OTHERS; do
  OTHER_SEED=${other%%:*}
  rest=${other#*:}
  OTHER_FORMAT=${rest%%:*}
  OTHER_OFF=${rest#*:}
  cp "$OTHER_SEED" "$TMP/other-evaluator"
  L=$(wc -c < "$TAPE" | tr -d ' ')
  printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) \
$(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of="$TMP/other-evaluator" bs=1 seek="$OTHER_OFF" conv=notrunc status=none
  dd if="$TAPE" of="$TMP/other-evaluator" bs=1 \
    seek=$((OTHER_OFF + 4)) conv=notrunc status=none
  python3 "$CONTAINER_PY" "$TMP/other-evaluator" --format "$OTHER_FORMAT" \
    --hole-off "$OTHER_OFF" --hole-size "$ALPHA_SEED_HOLE_SIZE" \
    --max-tape "$ALPHA_MAX_RAW_TAPE_SIZE" --stamped --tape "$TAPE" \
    --pristine "$OTHER_SEED" || fail "non-host stamped evaluator contract"
done
echo "acceptance ok — the evaluator tape satisfies the native-container contract in every audited seed"

if [ "$ALPHA_SEED_EXECUTABLE" = 1 ]; then
  echo "--- native execution receipt ---"
  printf '(def main () Int 42)\n' > "$TMP/acceptance.gamma"
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/invoke.py" \
    --evaluator "$TMP/evaluator" --source "$TMP/acceptance.gamma" \
    --output "$TMP/receipt" --timeout 30 ||
    fail "seeded evaluator did not accept the minimal source"
  printf '*' > "$TMP/expected"
  cmp "$TMP/receipt" "$TMP/expected" ||
    fail "seeded evaluator receipt changed"
  echo "execution ok — native receipt byte-exact"
else
  echo "execution skipped — host cannot exec an audited seed (seed-exec hosts run it)"
fi

echo "Gamma seed native acceptance: stamped evaluator satisfies the native-container contract on every audited seed"
