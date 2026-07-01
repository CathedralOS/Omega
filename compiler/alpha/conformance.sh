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
cd "$(dirname "$0")"
. ./seed_env.sh
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
SEED="$ALPHA_SEED"
PASS=0; FAIL=0

# tc NAME EXPECT_EXIT EXPECT_OUT STDIN HEX...
#   EXPECT_OUT="" -> don't check stdout ; STDIN="" -> empty stdin (EOF)
tc() {
  name="$1"; exp_exit="$2"; exp_out="$3"; stdin="$4"; shift 4
  echo "$*" | tr -d ' \n' | xxd -r -p > "$TMP/tape"
  stamp_seed "$TMP/tape" "$SEED" "$TMP/exe" >/dev/null 2>&1
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

echo ""
echo "alpha conformance ($SEED): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
