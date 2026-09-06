#!/usr/bin/env sh
# Alpha VM conformance suite. Runs hand-built bytecode tapes that pin each of the
# 21 opcodes and their edges (signedness, EOF, traps) against the HOST's alpha
# seed (selected by seed_env.sh), checking exit code and stdout. Any faithful
# seed — x64, arm64, or a future ISA — must pass all cases; this is the diamond
# made systematic and edge-covering. Tapes are explicit bytecode (not assembler
# output) so a broken assembler cannot mask a broken VM.
#
# Encoding: opcodes 1 byte; reg operands 1 byte; imm/addr 8 bytes LE. addr is an
# absolute tape offset (mem[0] = tape[0]). Trap (unknown opcode / div /0 or
# INT_MIN/-1) raises SIGILL -> shell exit 132 (128+4).
TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$TEST_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
cd "$TEST_DIR"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
PASS=0; FAIL=0

# tc NAME EXPECT_EXIT EXPECT_OUT STDIN HEX...
#   EXPECT_OUT="" -> don't check stdout ; STDIN="" -> empty stdin (EOF)
tc() {
  name="$1"; exp_exit="$2"; exp_out="$3"; stdin="$4"; shift 4
  echo "$*" | tr -d ' \n' | xxd -r -p > "$TMP/tape"
  if ! stamp_seed "$TMP/tape" "$SEED" "$TMP/exe" >/dev/null 2>&1; then
    FAIL=$((FAIL+1)); echo "  FAIL $name : seed stamping failed"; return
  fi
  out=$(printf '%s' "$stdin" | "$TMP/exe" 2>/dev/null); got_exit=$?
  ok=1
  [ "$got_exit" = "$exp_exit" ] || ok=0
  [ -z "$exp_out" ] || [ "$out" = "$exp_out" ] || ok=0
  if [ "$ok" = 1 ]; then PASS=$((PASS+1)); # echo "  ok   $name"
  else FAIL=$((FAIL+1)); echo "  FAIL $name : exit=$got_exit (want $exp_exit) out='$out' (want '$exp_out')"; fi
}

# --- per-opcode + edges ---
# 01 imm, 00 halt : exit = low byte of vregs[rD]
tc imm_halt        42  "" "" 01 00 2a00000000000000  00 00
tc halt_low8        5  "" "" 01 00 0501000000000000  00 00          # 261 -> low8 5
# 02 mov
tc mov              9  "" "" 01 01 0900000000000000  02 00 01  00 00
# 03 add  04 sub  05 mul
tc add             42  "" "" 01 00 2800000000000000 01 01 0200000000000000 03 00 01 00 00
tc sub             42  "" "" 01 00 3200000000000000 01 01 0800000000000000 04 00 01 00 00
tc mul             42  "" "" 01 00 0600000000000000 01 01 0700000000000000 05 00 01 00 00
# 06 div (signed, trunc toward zero)  07 mod (remainder sign = dividend)
tc div             14  "" "" 01 00 6400000000000000 01 01 0700000000000000 06 00 01 00 00
tc div_neg        253  "" "" 01 00 f9ffffffffffffff 01 01 0200000000000000 06 00 01 00 00   # -7/2 = -3 -> 253
tc mod_neg        255  "" "" 01 00 f9ffffffffffffff 01 01 0200000000000000 07 00 01 00 00   # -7%2 = -1 -> 255
tc mod_pos          1  "" "" 01 00 0700000000000000 01 01 0300000000000000 07 00 01 00 00   # 7%3 = 1
tc div_zero_trap  132  "" "" 01 00 0500000000000000 01 01 0000000000000000 06 00 01 00 00
tc div_ovf_trap   132  "" "" 01 00 0000000000000080 01 01 ffffffffffffffff 06 00 01 00 00   # INT_MIN / -1
# 09 storeb 08 loadb : mem[100]=200 ; read it back
tc storeb_loadb   200  "" "" 01 00 6400000000000000 01 01 c800000000000000 09 00 01 08 02 00 00 02
# 0B store 0A load : mem[256]=0x1122334455667788 ; low byte 0x88=136
tc store_load     136  "" "" 01 00 0001000000000000 01 01 8877665544332211 0b 00 01 0a 02 00 00 02
# 0C jmp : skip an imm r0,99
tc jmp             42  "" "" 01 00 2a00000000000000 0c 1d00000000000000 01 00 6300000000000000 00 00
# 0D jz taken (r0==0)
tc jz_taken        42  "" "" 01 00 0000000000000000 01 01 2a00000000000000 0d 00 2800000000000000 01 01 6300000000000000 00 01
# 0D jz not taken (r0==1)
tc jz_nottaken      7  "" "" 01 00 0100000000000000 01 01 2a00000000000000 0d 00 2800000000000000 01 01 0700000000000000 00 01
# 0E jnz taken (r0==1)
tc jnz_taken       42  "" "" 01 00 0100000000000000 01 01 2a00000000000000 0e 00 2800000000000000 01 01 6300000000000000 00 01
# 0F jlt SIGNED (-1 < 1 ; unsigned would fail)
tc jlt_signed      42  "" "" 01 00 ffffffffffffffff 01 01 0100000000000000 01 02 2a00000000000000 0f 00 01 3300000000000000 01 02 6300000000000000 00 02
# 10 jeq taken
tc jeq             42  "" "" 01 00 0500000000000000 01 01 0500000000000000 01 02 2a00000000000000 10 00 01 3300000000000000 01 02 6300000000000000 00 02
# 11 read : EOF -> -1 (low8 255)
tc read_eof       255  "" "" 11 00 00 00
# 11 read : a byte 'A'=65
tc read_byte       65  "" "A" 11 00 00 00
# 12 write : emit "Hi" ; exit low8 of last = 105
tc write          105  "Hi" "" 01 00 4800000000000000 12 00 01 00 6900000000000000 12 00 00 00
# 13 call / 14 ret : subroutine sets r0=42 and returns
tc call_ret        42  "" "" 13 0b00000000000000 00 00 01 00 2a00000000000000 14
# unknown opcode -> trap
tc unknown_trap   132  "" "" ff

# AlphaBootstrapV3 adds semantic memory above the unchanged stack origin.
# All accesses below are in [0, 0x40000000); no out-of-range result is assumed.
tc upper_origin_zero 0 "" "" 01 00 0000001000000000 08 01 00 00 01
tc upper_middle_zero 0 "" "" 01 00 0000002000000000 08 01 00 00 01
tc upper_final_zero  0 "" "" 01 00 ffffff3f00000000 08 01 00 00 01
tc upper_origin_byte 165 "" "" 01 00 0000001000000000 01 01 a500000000000000 09 00 01 08 02 00 00 02
tc upper_final_byte 231 "" "" 01 00 ffffff3f00000000 01 01 e700000000000000 09 00 01 08 02 00 00 02
# Full-word comparison checks all eight bytes ending at the selected extent.
tc upper_final_word 42 "" "" 01 00 f8ffff3f00000000 01 01 8877665544332211 0b 00 01 0a 02 00 01 03 0100000000000000 10 01 02 3100000000000000 00 03 01 03 2a00000000000000 00 03
# The first call still stores return offset 9 at 0x0ffffff8, then returns.
tc unchanged_stack_origin 9 "" "" 13 0b00000000000000 00 00 01 01 f8ffff0f00000000 0a 00 01 14

# AlphaBootstrapV3 realization profile: the exact maximum raw tape must fit the
# physical hole, round-trip unchanged, and execute. The adjacent raw byte must
# be rejected before the caller's destination is touched.
dd if=/dev/zero of="$TMP/exact-capacity.tape" bs="$ALPHA_MAX_RAW_TAPE_SIZE" count=1 2>/dev/null
printf '\000\000' | dd of="$TMP/exact-capacity.tape" bs=1 seek=0 conv=notrunc status=none
capacity_ok=1
stamp_seed "$TMP/exact-capacity.tape" "$SEED" "$TMP/exact-capacity.exe" >/dev/null 2>&1 || capacity_ok=0
if [ "$capacity_ok" = 1 ]; then
  tape_in_seed "$TMP/exact-capacity.exe" > "$TMP/exact-capacity.framed" 2>/dev/null || capacity_ok=0
fi
if [ "$capacity_ok" = 1 ]; then
  [ "$(wc -c < "$TMP/exact-capacity.framed" | tr -d ' ')" -eq "$ALPHA_SEED_HOLE_SIZE" ] || capacity_ok=0
  [ "$(od -An -tu4 -N4 "$TMP/exact-capacity.framed" | tr -dc 0-9)" -eq "$ALPHA_MAX_RAW_TAPE_SIZE" ] || capacity_ok=0
  dd if="$TMP/exact-capacity.framed" of="$TMP/exact-capacity.extracted" bs=1 skip=4 status=none
  cmp -s "$TMP/exact-capacity.tape" "$TMP/exact-capacity.extracted" || capacity_ok=0
fi
if [ "$capacity_ok" = 1 ]; then
  "$TMP/exact-capacity.exe" >/dev/null 2>&1
  [ "$?" -eq 0 ] || capacity_ok=0
fi

cp "$TMP/exact-capacity.tape" "$TMP/adjacent-capacity.tape"
printf '\000' >> "$TMP/adjacent-capacity.tape"
printf 'preserve-on-refusal' > "$TMP/adjacent-capacity.exe"
cp "$TMP/adjacent-capacity.exe" "$TMP/adjacent-capacity.before"
if stamp_seed "$TMP/adjacent-capacity.tape" "$SEED" "$TMP/adjacent-capacity.exe" >/dev/null 2>&1; then
  capacity_ok=0
fi
cmp -s "$TMP/adjacent-capacity.before" "$TMP/adjacent-capacity.exe" || capacity_ok=0

# A stale V1-sized container must fail before copy, and extraction must not trust
# an embedded length outside the selected profile even when the file is large.
dd if="$SEED" of="$TMP/stale-seed" bs=1 count=$((HOLE_OFF + ALPHA_SEED_HOLE_SIZE - 1)) status=none
printf 'preserve-stale-refusal' > "$TMP/stale-destination"
cp "$TMP/stale-destination" "$TMP/stale-destination.before"
if stamp_seed "$TMP/exact-capacity.tape" "$TMP/stale-seed" "$TMP/stale-destination" >/dev/null 2>&1; then
  capacity_ok=0
fi
cmp -s "$TMP/stale-destination.before" "$TMP/stale-destination" || capacity_ok=0

cp "$SEED" "$TMP/bad-embedded-length"
bad_length=$((ALPHA_MAX_RAW_TAPE_SIZE + 1))
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((bad_length & 255)) $(((bad_length >> 8) & 255)) $(((bad_length >> 16) & 255)) $(((bad_length >> 24) & 255)))" \
  | dd of="$TMP/bad-embedded-length" bs=1 seek="$HOLE_OFF" conv=notrunc status=none
if tape_in_seed "$TMP/bad-embedded-length" > "$TMP/bad-embedded-output" 2>/dev/null; then
  capacity_ok=0
fi
[ ! -s "$TMP/bad-embedded-output" ] || capacity_ok=0

if [ "$capacity_ok" = 1 ]; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1)); echo "  FAIL AlphaBootstrapV3 exact/adjacent capacity"
fi

echo ""
echo "alpha conformance ($SEED): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
