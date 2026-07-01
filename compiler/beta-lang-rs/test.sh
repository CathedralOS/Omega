#!/usr/bin/env sh
# beta-lang self-check gate: build every example + the calculator self-check
# (slice 6) through the real pipeline (Beta -> asm -> tape -> host seed) and
# assert exit codes / stdout. Proves the Beta compiler is working end to end.
cd "$(dirname "$0")"
PASS=0; FAIL=0
chk() { # name expect_exit  (expr via stdin in $3 if calc)
  if [ "$2" = "$3" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $1"; fi
}
build() { sh build.sh "examples/$1.beta" >/dev/null 2>&1 || { echo "  BUILD FAIL $1"; FAIL=$((FAIL+1)); return 1; }; }

# value examples: exit code == expected
for pair in answer:42 double:42 states:55 calls:10 factorial:120 fib:55 sumto:55 arrays:30 bytes:131 args3:40 args4:10; do
  name=${pair%:*}; want=${pair#*:}
  build "$name" || continue
  "./build/$name.exe" </dev/null; got=$?
  if [ "$got" = "$want" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $name = $got (want $want)"; fi
done

# calc self-check: stdin expression -> printed decimal + exit code (low byte)
build calc || true
for pair in "2+3*4=14" "(2+3)*4=20" "100-58=42" "2*(3+4)*5=70" "  12 +  30  =42" "2*3+4*5=26" "100/7=14" "(1+2)*(3+4)=21" "7=7"; do
  expr=${pair%=*}; want=${pair#*=}
  out=$(printf '%s' "$expr" | ./build/calc.exe); code=$?
  if [ "$out" = "$want" ] && [ "$code" = "$((want & 255))" ]; then PASS=$((PASS+1));
  else FAIL=$((FAIL+1)); echo "  FAIL calc '$expr' -> '$out' (exit $code, want $want)"; fi
done
echo "beta-lang: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
