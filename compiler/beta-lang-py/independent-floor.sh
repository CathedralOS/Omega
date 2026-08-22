#!/usr/bin/env sh
# INDEPENDENT FLOOR — the whole alpha->beta->bc floor has a complete second realization, in Python, and it
# runs real programs identically to the lineage. For each random program, build+run it TWO independent ways
# and require the same exit code and stdout:
#   python  : bc2.py (Beta -> asm) | asm_ref.py (asm -> tape) | alpha_ref.py (tape -> run)   [zero lineage binaries]
#   lineage : bc (Beta -> asm)     | assembler.alpha (asm -> tape) | the seed VM (tape -> run)
# Historical whole-floor comparison. Each stage is also covered by narrower
# reference checks (the legacy compiler comparison, ../beta/asm-diamond.sh,
# ../alpha/diamond-py.sh); this composes all three on a FRESH random corpus, so a divergence anywhere in the
# independent floor — compiler, assembler, or VM — shows up as a disagreement. It is the end-to-end
# This remains optional diagnostic tooling; it is not a trust or release gate.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "independent floor: skipped (python3 absent)"; exit 0; }
command -v cargo   >/dev/null 2>&1 || { echo "independent floor: skipped (no cargo for the on-ramp)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
BC=../beta-lang-rs/build/bc.exe
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "independent floor: bc build failed"; exit 1; }
[ -x "$BC" ] && [ -x "$ASM" ] || { echo "independent floor: skipped (bc/assembler missing)"; exit 0; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
N=${1:-50}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((990000 + i))
  python3 beta-fuzz-gen.py "$s" > "$T/p.beta"
  # the ALL-PYTHON floor: bc2.py -> asm_ref.py -> alpha_ref.py (alpha_ref runs a raw tape file, no stamping)
  if python3 bc2.py < "$T/p.beta" > "$T/py.asm" 2>"$T/e" \
     && python3 ../beta/asm_ref.py < "$T/py.asm" > "$T/py.tape" 2>>"$T/e"; then
    po=$(python3 ../alpha/alpha_ref.py "$T/py.tape" </dev/null 2>/dev/null); pc=$?
  else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : python floor build error: $(cat "$T/e")"; i=$((i+1)); continue; fi
  # the ALL-LINEAGE floor: bc -> assembler -> stamped seed (reap SIGILL traps quietly)
  if "$BC" < "$T/p.beta" > "$T/ln.asm" 2>/dev/null && "$ASM" < "$T/ln.asm" > "$T/ln.tape" 2>/dev/null \
     && stamp_seed "$T/ln.tape" "$SEED" "$T/ln.exe" >/dev/null 2>&1; then
    lo=$(sh -c '"$1"; exit $?' _ "$T/ln.exe" </dev/null 2>/dev/null); lc=$?
  else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : lineage floor build error"; i=$((i+1)); continue; fi
  if [ "$pc" = "$lc" ] && [ "$po" = "$lo" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : python=(out='$po' rc=$pc)  lineage=(out='$lo' rc=$lc)"; sed 's/^/    /' "$T/p.beta"; fi
  i=$((i + 1))
done
echo "independent floor (bc2.py->asm_ref.py->alpha_ref.py == bc->assembler->seed over $N random programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
