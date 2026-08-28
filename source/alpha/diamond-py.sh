#!/usr/bin/env sh
# SEED DIAMOND, third point — the independent Python reference VM (alpha_ref.py) agrees with the host seed.
#
# The hand-authored seed VMs are checked against written semantics, but assembly is
# hard to audit. alpha_ref.py is a third, independent implementation, short enough to read against
# SEMANTICS.md. This gate runs a corpus through BOTH the host seed AND alpha_ref.py and asserts they agree
# on exit code and stdout. A bug in either path surfaces as a disagreement.
# alpha_ref.py is UNTRUSTED and checked; the
# runtime lineage never runs it.
#
# Corpus = the opcode EDGES that real code rarely hits (signedness, traps, EOF) + REAL bc-compiled programs
# (call/ret/frames/memory/IO exercised through actual generated code). The seed's own per-opcode battery is
# conformance.sh; this gate's job is cross-implementation agreement, not re-pinning every opcode.
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "diamond-py SKIP — no python3"; exit 0; }
. "${OMEGA_PATH_BETA_COMPILER}/artifact_env.sh"
SEED="$ALPHA_SEED"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
PASS=0; FAIL=0

cmp_tape() {  # name  tapefile  stdin  (set +e around the runs: programs exit nonzero by design)
  stamp_seed "$2" "$SEED" "$T/exe" >/dev/null 2>&1
  set +e
  so=$(printf '%s' "$3" | "$T/exe" 2>/dev/null); sc=$?
  po=$(printf '%s' "$3" | python3 alpha_ref.py "$2" 2>/dev/null); pc=$?
  set -e
  if [ "$sc" = "$pc" ] && [ "$so" = "$po" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : seed=(rc=$sc out='$so')  ref=(rc=$pc out='$po')"; fi
}
hex() { echo "$2" | tr -d ' \n' | xxd -r -p > "$T/$1.tape"; cmp_tape "$1" "$T/$1.tape" "$3"; }

# --- opcode EDGES (signedness, traps, EOF) — where independent implementations most easily disagree ---
hex imm_halt      "01 00 2a00000000000000  00 00" ""
hex halt_low8     "01 00 0501000000000000  00 00" ""                                             # 261 -> 5
hex div_neg       "01 00 f9ffffffffffffff 01 01 0200000000000000 06 00 01 00 00" ""              # -7/2=-3 -> 253
hex mod_neg       "01 00 f9ffffffffffffff 01 01 0200000000000000 07 00 01 00 00" ""              # -7%2=-1 -> 255
hex div0_trap     "01 00 0500000000000000 01 01 0000000000000000 06 00 01 00 00" ""              # /0 -> 132
hex ovf_trap      "01 00 0000000000000080 01 01 ffffffffffffffff 06 00 01 00 00" ""              # INT_MIN/-1 -> 132
hex jlt_signed    "01 00 ffffffffffffffff 01 01 0100000000000000 0f 00 01 2100000000000000 01 02 0000000000000000 00 02 01 02 0100000000000000 00 02" ""  # -1 <s 1
hex read_eof      "11 00 00 00" ""                                                               # read at EOF -> 0xFF.. ; halt low8 = 0xFF=255
hex bad_opcode    "ff 00" ""                                                                     # unknown -> trap 132

# --- REAL bc-compiled programs: call/ret/frames/recursion/memory/IO through actual generated code ---
BC="$T/bc.exe"
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
stamp_beta_compiler "$BC" >/dev/null 2>&1 || { echo "seed diamond: lattice bc artifact unavailable"; exit 1; }
mkbeta() { "$BC" < "$1" > "$T/b.asm" 2>/dev/null && "$ASM" < "$T/b.asm" > "$T/$2.tape" 2>/dev/null; }
if [ -x "$BC" ] && [ -x "$ASM" ]; then
  printf 'proc fact(n){ state c{ to r when (n>1) return 1 } state r{ return n*fact(n-1) } }\nproc main(){ return fact(5) }\n' > "$T/f.beta"
  printf 'proc gcd(a,b){ state c{ to d when (b==0) return gcd(b, a%%b) } state d{ return a } }\nproc main(){ return gcd(48,36) }\n' > "$T/g.beta"
  printf 'proc main(){ let c=read_byte()  state l{ to b when (c>=0) return 0 } state b{ write_byte(c) c=read_byte() to l } }\n' > "$T/e.beta"
  printf 'proc n(x){ state b{ to r when (x>=10) to d } state r{ n(x/10) to d } state d{ write_byte(x%%10+48) return 0 } }\nproc main(){ n(1234) return 0 }\n' > "$T/p.beta"
  mkbeta "$T/f.beta" fact && cmp_tape "real: fact(5) recursion"       "$T/fact.tape" ""
  mkbeta "$T/g.beta" gcd  && cmp_tape "real: gcd(48,36) mod/recursion" "$T/gcd.tape"  ""
  mkbeta "$T/e.beta" echo && cmp_tape "real: echo read/write/loop"     "$T/echo.tape" "diamond!"
  mkbeta "$T/p.beta" pn   && cmp_tape "real: print_num 1234"           "$T/pn.tape"   ""
else
  echo "  (skipped real-program cases — bc/assembler not available)"
fi

echo "seed diamond (independent Python reference VM alpha_ref.py agrees with the host seed on edges + real programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
