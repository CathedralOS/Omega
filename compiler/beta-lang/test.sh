#!/usr/bin/env sh
# Gate for bc.beta — the Beta compiler written in Beta. Builds bc (via the Rust
# on-ramp), then uses bc AS a compiler: feeds it `proc main() { return <expr> }`
# programs, assembles + runs bc's emitted asm, and checks the exit code equals
# the arithmetic value. (Slice 1: arithmetic.)
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED

# build bc.exe = the bc.beta compiler, lowered through the on-ramp
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
BC=../beta-lang-rs/build/bc.exe
echo "bc tape: $(wc -c < ../beta-lang-rs/build/bc.tape | tr -d ' ') B (hole 32768)"

PASS=0; FAIL=0
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
run() { # expr expected
  printf 'proc main() { return %s }\n' "$1" | "$BC" > "$T/p.asm" 2>/dev/null
  "$ASM" < "$T/p.asm" > "$T/p.tape" 2>/dev/null
  stamp_seed "$T/p.tape" "$SEED" "$T/p.exe" >/dev/null 2>&1
  "$T/p.exe" </dev/null; got=$?
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL '$1' -> $got (want $2)"; fi
}
run "6 * 7" 42
run "2 + 3 * 4" 14
run "(2 + 3) * 4" 20
run "100 - 58" 42
run "2 * (3 + 4) * 5" 70
run "100 / 7" 14
run "17 % 5" 2
run "1 + 2 + 3 + 4 + 5" 15
echo "bc.beta (slice 1, arithmetic): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
