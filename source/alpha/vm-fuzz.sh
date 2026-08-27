#!/usr/bin/env sh
# VM FUZZER — broad random differential testing of the trust root. Generates many random arithmetic tapes
# (vm-fuzz-gen.py, deterministic) and checks that the host SEED VM and the independent reference alpha_ref.py
# agree on exit code for every one. Three independent realizations of the 21-op semantics; a single
# disagreement on 64-bit wraparound / signed div-mod / trap edges exposes a VM or reference bug
# backdoor. This is the systematic version of diamond-py.sh's hand-picked edge cases. Deterministic (fixed
# base seed). Needs python3; skips cleanly without.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "vm fuzz: skipped (python3 absent)"; exit 0; }
. ./seed_env.sh
SEED="$ALPHA_SEED"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
N=${1:-80}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((424242 + i))
  python3 vm-fuzz-gen.py "$s" > "$T/tape"
  stamp_seed "$T/tape" "$SEED" "$T/exe" >/dev/null 2>&1
  # a trapping tape kills the seed with SIGILL; reap it inside an inner sh whose stderr is redirected so the
  # shell's "Illegal instruction" job message is swallowed, while the exit code (132) still propagates.
  sh -c '"$1"; exit $?' _ "$T/exe" </dev/null >/dev/null 2>&1; sc=$?
  python3 alpha_ref.py "$T/tape" </dev/null >/dev/null 2>&1; pc=$?
  if [ "$sc" = "$pc" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1))
    echo "  FAIL seed=$s : host seed exit=$sc, alpha_ref exit=$pc"
    xxd -p "$T/tape" | tr -d '\n' | sed 's/^/    tape: /'; echo
  fi
  i=$((i + 1))
done
echo "vm fuzz (host seed VM == independent reference alpha_ref.py over $N random arithmetic tapes): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
