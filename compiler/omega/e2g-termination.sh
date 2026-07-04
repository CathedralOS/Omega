#!/usr/bin/env sh
# E2G TERMINATION CANARY — omega2gamma must TERMINATE on EVERY sample, supported or not.
#
# The translator is an untrusted state machine; on constructs outside its fragment it must refuse
# loudly (emit an E2G-UNSUPPORTED marker, which no downstream parser accepts) — never scan forever.
# Two real divergences motivated this gate: write_line with a non-literal argument, and a bare
# terminal expression (`{ 0 }`), both of which spun unguarded scans at end-of-input (one cost an
# 8h45m hung job). Every sample runs under a hard alarm; a timeout is a FAIL naming the sample.
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "e2g-termination FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < omega2gamma.beta > "$T/e.asm" 2>/dev/null \
  && "$ASM" < "$T/e.asm" > "$T/e.tape" 2>/dev/null \
  && stamp_seed "$T/e.tape" "$SEED" "$T/e2g.exe" >/dev/null 2>&1 \
  || { echo "e2g-termination FAIL — build omega2gamma.beta"; exit 1; }

PASS=0; FAIL=0
for d in ../../samples/*/; do
  s=$(basename "$d")
  [ -f "$d/main.omg" ] || continue
  perl -e 'alarm 20; exec @ARGV' "$T/e2g.exe" < "$d/main.omg" > /dev/null 2>&1
  rc=$?
  if [ "$rc" = 142 ] || [ "$rc" = 137 ]; then
    FAIL=$((FAIL+1)); echo "  FAIL $s : omega2gamma did not terminate (rc=$rc)"
  else
    PASS=$((PASS+1))
  fi
done
echo "e2g termination canary (the translator halts on every sample, supported or refused): $PASS ok, $FAIL hung"
[ "$FAIL" = 0 ] && [ "$PASS" -gt 0 ]
