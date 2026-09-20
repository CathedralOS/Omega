#!/usr/bin/env sh
# SEED PARITY — the committed Linux x86-64 container (alpha_x64_linux)
# agrees with the independent Python reference VM (alpha_ref.py).
#
# diamond-py.sh asserts host-seed ↔ reference agreement, but only on hosts
# seed_env.sh admits (macOS arm64, Windows x64). The third audited container
# executes on Linux x86-64 — container.sh already binds its identity and hole
# contract host-free, and bounds.py runs it natively — yet no gate asserted
# its parity with the reference. This gate is that assertion: every tape in
# the corpus runs through BOTH the stamped Linux container and alpha_ref.py,
# and any exit-code or stdout disagreement fails.
#
# The corpus joins the conformance battery's per-opcode cases with the
# diamond edge cases (signedness, traps, EOF, calls, the V5 memory extent,
# and the host-I/O register fixture) — the surfaces where independent
# implementations most easily diverge. It needs Linux x86-64 to execute the
# ELF container; other hosts report the standard unavailable refusal.
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$TEST_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
    echo "bootstrap paths: cannot find repository root from $TEST_DIR" >&2
    exit 2
  fi
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

command -v python3 >/dev/null 2>&1 || { echo "seed parity SKIP — no python3"; exit 0; }
[ "$(uname -s)-$(uname -m)" = "Linux-x86_64" ] || {
  echo "seed parity: alpha_x64_linux executes only on Linux x86-64" >&2
  exit 2
}

LIN_SEED="$OMEGA_PATH_ALPHA/alpha_x64_linux"
LIN_HOLE_OFF=12288
INVENTORY="bootstrap/0_alpha/README.md"

# The Linux seed's bound identity lives in the retention inventory; parse the
# row the same way container.sh does and bind it before any tape executes.
LIN_ROW=$(grep -F '`alpha_x64_linux`' "$OMEGA_REPO_ROOT/$INVENTORY") ||
  { echo "retention inventory lacks an alpha_x64_linux row" >&2; exit 1; }
LIN_SIZE=$(printf '%s\n' "$LIN_ROW" | sed -n 's/.*| *\([0-9,][0-9,]*\) *|.*/\1/p' | tr -d ',')
LIN_SHA=$(printf '%s\n' "$LIN_ROW" | sed -n 's/.*| *`\([0-9a-f]\{64\}\)` *|.*/\1/p')
[ -n "$LIN_SIZE" ] && [ -n "$LIN_SHA" ] ||
  { echo "alpha_x64_linux inventory row is not parseable" >&2; exit 1; }
require_bound_identity "alpha_x64_linux" "$LIN_SEED" "$LIN_SIZE" "$LIN_SHA" \
  "$INVENTORY" || { echo "seed parity: Linux container fails the bound identity" >&2; exit 1; }

REF="$TEST_DIR/reference/alpha_ref.py"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
PASS=0; FAIL=0

# stamp TAPE OUT : write [4-byte LE len][tape] into the Linux container's
# recorded hole — the same manual stamp container.sh applies to non-host
# seeds, since stamp_seed follows the seed_env-selected profile.
stamp_linux() {
  cp "$LIN_SEED" "$2" && chmod +x "$2"
  L=$(wc -c < "$1" | tr -d ' ')
  printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) \
$(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of="$2" bs=1 seek="$LIN_HOLE_OFF" conv=notrunc status=none
  dd if="$1" of="$2" bs=1 seek=$((LIN_HOLE_OFF + 4)) conv=notrunc status=none
}

# pc NAME TAPEFILE STDIN — assert seed and reference agree on exit + stdout.
pc() {
  stamp_linux "$2" "$T/exe" || { FAIL=$((FAIL+1)); echo "  FAIL $1 : linux seed stamping failed"; return; }
  set +e
  so=$(printf '%s' "$3" | "$T/exe" 2>/dev/null); sc=$?
  po=$(printf '%s' "$3" | python3 "$REF" "$2" 2>/dev/null); pc_=$?
  set -e
  if [ "$sc" = "$pc_" ] && [ "$so" = "$po" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : seed=(rc=$sc out='$so')  ref=(rc=$pc_ out='$po')"; fi
}
hex() { echo "$2" | tr -d ' \n' | xxd -r -p > "$T/$1.tape"; pc "$1" "$T/$1.tape" "$3"; }

# --- per-opcode parity (conformance battery cases) ---
hex imm_halt      "01 00 2a00000000000000  00 00" ""
hex halt_low8     "01 00 0501000000000000  00 00" ""
hex mov           "01 01 0900000000000000 02 00 01  00 00" ""
hex add           "01 00 2800000000000000 01 01 0200000000000000 03 00 01 00 00" ""
hex sub           "01 00 3200000000000000 01 01 0800000000000000 04 00 01 00 00" ""
hex mul           "01 00 0600000000000000 01 01 0700000000000000 05 00 01 00 00" ""
hex div           "01 00 6400000000000000 01 01 0700000000000000 06 00 01 00 00" ""
hex div_neg       "01 00 f9ffffffffffffff 01 01 0200000000000000 06 00 01 00 00" ""
hex mod_neg       "01 00 f9ffffffffffffff 01 01 0200000000000000 07 00 01 00 00" ""
hex mod_pos       "01 00 0700000000000000 01 01 0300000000000000 07 00 01 00 00" ""
hex div0_trap     "01 00 0500000000000000 01 01 0000000000000000 06 00 01 00 00" ""
hex ovf_trap      "01 00 0000000000000080 01 01 ffffffffffffffff 06 00 01 00 00" ""
hex storeb_loadb  "01 00 6400000000000000 01 01 c800000000000000 09 00 01 08 02 00 00 02" ""
hex store_load    "01 00 0001000000000000 01 01 8877665544332211 0b 00 01 0a 02 00 00 02" ""
hex jmp           "01 00 2a00000000000000 0c 1d00000000000000 01 00 6300000000000000 00 00" ""
hex jz_taken      "01 00 0000000000000000 01 01 2a00000000000000 0d 00 2800000000000000 01 01 6300000000000000 00 01" ""
hex jz_nottaken   "01 00 0100000000000000 01 01 2a00000000000000 0d 00 2800000000000000 01 01 0700000000000000 00 01" ""
hex jnz_taken     "01 00 0100000000000000 01 01 2a00000000000000 0e 00 2800000000000000 01 01 6300000000000000 00 01" ""
hex jlt_signed    "01 00 ffffffffffffffff 01 01 0100000000000000 01 02 2a00000000000000 0f 00 01 3300000000000000 01 02 6300000000000000 00 02" ""
hex jeq           "01 00 0500000000000000 01 01 0500000000000000 01 02 2a00000000000000 10 00 01 3300000000000000 01 02 6300000000000000 00 02" ""
hex read_eof      "11 00 00 00" ""
hex read_byte     "11 00 00 00" "A"
hex write         "01 00 4800000000000000 12 00 01 00 6900000000000000 12 00 00 00" ""
hex call_ret      "13 0b00000000000000 00 00 01 00 2a00000000000000 14" ""
hex unknown_trap  "ff" ""

# --- host-I/O scratch isolation + V5 memory extent (conformance fixtures) ---
hex io_registers "$(sed 's/;.*//' "$TEST_DIR/io-registers.hex")" "AB"
hex upper_origin_zero "01 00 0000004000000000 08 01 00 00 01" ""
hex upper_middle_zero "01 00 0000000010000000 08 01 00 00 01" ""
hex upper_final_zero  "01 00 ffffffff01000000 08 01 00 00 01" ""
hex upper_origin_byte "01 00 0000004000000000 01 01 a500000000000000 09 00 01 08 02 00 00 02" ""
hex upper_final_byte  "01 00 ffffffff01000000 01 01 e700000000000000 09 00 01 08 02 00 00 02" ""
hex upper_final_word  "01 00 f8ffffff01000000 01 01 8877665544332211 0b 00 01 0a 02 00 01 03 0100000000000000 10 01 02 3100000000000000 00 03 01 03 2a00000000000000 00 03" ""
hex unchanged_stack_origin "13 0b00000000000000 00 00 01 01 f8ffff0f00000000 0a 00 01 14" ""

echo "seed parity (alpha_x64_linux agrees with alpha_ref.py): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
