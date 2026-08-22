#!/usr/bin/env sh
# BETA COMPILER CORRECTNESS FUZZ — random differential testing of bc's SEMANTICS (not just its
# reproducibility or cross-compiler agreement). For each random program, run it two
# independent ways and require agreement on exit code AND stdout:
#   interpret : beta_interp.py runs the Beta source directly (a second, independent definition of Beta)
#   compile   : bc compiles it to Alpha bytecode, which the seed VM runs
# A disagreement means bc miscompiled the program (or the interpreter is wrong — either way, a loud signal
# worth investigating, never a silent pass). Both are UNTRUSTED and checked against each other.
# Deterministic (fixed base seed). Needs python3 + the bc/assembler build; skips cleanly otherwise.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "beta correctness fuzz: skipped (python3 absent)"; exit 0; }
command -v cargo   >/dev/null 2>&1 || { echo "beta correctness fuzz: skipped (no cargo for the on-ramp)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
BC=../beta-lang-rs/build/bc.exe
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "beta correctness fuzz: bc build failed"; exit 1; }
[ -x "$BC" ] && [ -x "$ASM" ] || { echo "beta correctness fuzz: skipped (bc/assembler missing)"; exit 0; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
N=${1:-60}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((770000 + i))
  python3 beta-fuzz-gen.py "$s" > "$T/p.beta"
  # interpret directly
  io=$(python3 beta_interp.py "$T/p.beta" </dev/null 2>/dev/null); ic=$?
  # compile with bc, run on the seed VM (reap SIGILL traps quietly, propagate the 132 exit)
  if "$BC" < "$T/p.beta" > "$T/p.asm" 2>/dev/null && "$ASM" < "$T/p.asm" > "$T/p.tape" 2>/dev/null \
     && stamp_seed "$T/p.tape" "$SEED" "$T/p.exe" >/dev/null 2>&1; then
    co=$(sh -c '"$1"; exit $?' _ "$T/p.exe" </dev/null 2>/dev/null); cc=$?
  else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : bc/assembler could not build the program"; i=$((i+1)); continue
  fi
  if [ "$ic" = "$cc" ] && [ "$io" = "$co" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1))
    echo "  FAIL seed=$s : interpret=(out='$io' rc=$ic)  compiled=(out='$co' rc=$cc)"
    sed 's/^/    /' "$T/p.beta"
  fi
  i=$((i + 1))
done
echo "beta compiler correctness fuzz (interpret == compile+run over $N random programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
