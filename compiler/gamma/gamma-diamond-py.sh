#!/usr/bin/env sh
# GAMMA MEANING DIAMOND — the independent reference evaluator (gamma_ref.py) agrees with interp.beta.
#
# interp.beta is the canonical definition of what Gamma programs MEAN; delta proves theorems ABOUT that
# meaning, so its correctness underpins the proof edifice. Its arithmetic is cross-checked against delta's
# normalizer (seam-fuzz.sh) and it is checked on the omega samples (kernel-diamond) — but its ADT / match /
# recursion EVALUATION has no independent implementation to diamond against. gamma_ref.py is that
# implementation; this gate runs random Gamma programs (gamma-fuzz-gen.py) through BOTH and asserts they
# agree on the printed result and exit code. A disagreement over ADTs / match / recursion / signed
# arithmetic / traps would expose a meaning bug in one of them. Deterministic; needs python3; skips cleanly.
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "gamma diamond (py): skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "gamma diamond (py): bc build failed"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < interp.beta > "$T/g.asm" 2>/dev/null && "$ASM" < "$T/g.asm" > "$T/g.tape" 2>/dev/null \
  && stamp_seed "$T/g.tape" "$SEED" "$T/g.exe" >/dev/null 2>&1 || { echo "gamma diamond (py): interp build failed"; exit 1; }
G="$T/g.exe"

N=${1:-100}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((660000 + i))
  python3 gamma-fuzz-gen.py "$s" > "$T/p.gamma"
  set +e
  ro=$(python3 gamma_ref.py < "$T/p.gamma" 2>/dev/null); rc=$?
  io=$("$G" < "$T/p.gamma" 2>/dev/null); ic=$?
  set -e
  if [ "$rc" = "$ic" ] && [ "$ro" = "$io" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : gamma_ref=(out'$ro' rc$rc) interp=(out'$io' rc$ic)"; sed 's/^/    /' "$T/p.gamma"; fi
  i=$((i + 1))
done
echo "gamma meaning diamond (independent gamma_ref.py agrees with interp.beta over $N random ADT/recursion programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
