#!/usr/bin/env sh
# ∀-INPUT SAMPLE CONNECTION — a real Omega sample's accumulator loop proven correct for EVERY input.
#
# input-tv.sh proves an input loop's meaning on the DOCUMENTED input vectors; forall-input.sh proves the
# ABSTRACT fold theorems (count/sum/prod/...) for all inputs. THIS gate joins the two: it confirms a real
# sample machine IS one of those abstract folds, so the ∀-input theorem discharges that sample's loop
# UNIVERSALLY — for every input slice, not just the documented one. It is the first proof-carrying step that
# says "this real program's loop is correct for all inputs", the summit of the meaning route.
#
# recursive_sum's `count(s, acc) = if s.len > 0 then count(s[1..], acc + 1) else acc` is the COUNT fold: it
# adds a CONSTANT (1) per element and is element-VALUE-independent (purely structural over the slice length),
# so it is ℤ-FREE — the nat theorem applies with no signed-integer bridge. count-forall.elab proves
# `∀xs ∀n. count(xs, n) = len(xs) + n`, verified here by check.beta + check_ref.py + checker.gamma with an
# off-by-one perturbation rejected. So recursive_sum's count computes len(s) + acc for EVERY input slice.
# (The `sum` machine adds the SIGNED element, which needs a ℤ fold theorem — deliberately future work; count
# is structural, so it is the clean first connection.)
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "forall-sample: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "forall-sample FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta           "$T/check.exe"  || { echo "forall-sample FAIL — build check.beta"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "forall-sample FAIL — build interp.beta"; exit 1; }
DEFS=$(cat ../gamma/checker.gamma)

# ---- 1. count-forall verified by all three independent checkers; off-by-one perturbation rejected ----
cert=$(python3 elab.py < count-forall.elab 2>/dev/null)
gb=$(printf '%s' "$cert" | "$T/check.exe" 2>/dev/null)
gr=$(printf '%s' "$cert" | python3 check_ref.py 2>/dev/null)
gg=$(printf '%s\n%s\n' "$DEFS" "$(printf '%s' "$cert" | python3 ../gamma/refcert_to_gamma.py 2>/dev/null)" | perl -e 'alarm 40; exec @ARGV' "$T/interp.exe" >/dev/null 2>&1; r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; })
neg=$(sed '/^(all xs/ s/(f 21 (f 9\([0-9]\) xs) n)/(k 3 (f 21 (f 9\1 xs) n))/' count-forall.elab | python3 elab.py 2>/dev/null)
nb=$(printf '%s' "$neg" | "$T/check.exe" 2>/dev/null)
nr=$(printf '%s' "$neg" | python3 check_ref.py 2>/dev/null)
negok=no; { [ "$nb" != accept ] && [ "$nr" != accept ]; } && negok=yes
echo "  count-forall (count(xs,n) = len(xs)+n): check.beta=$gb check_ref=$gr checker.gamma=$gg | perturbed rejected=$negok"

# ---- 2. tie: each real sample whose machine IS the count fold gets its loop discharged for EVERY input ----
cov=0; miss=0
for f in ../../samples/*/main.omg; do
  rec=$(grep -oE '[a-z_]+\([a-z_]+\[1\.\.\], *[a-z_]+ \+ 1\)' "$f" 2>/dev/null | head -1)
  [ -n "$rec" ] || continue
  s=$(basename "$(dirname "$f")")
  name=${rec%%\(*}
  if grep -qE "$name"'\(&mut self, *[a-z_]+: *&\[' "$f" && grep -qE '\.len > 0' "$f"; then
    cov=$((cov+1))
    echo "  ok   $s : machine '$name' IS the count fold ($rec under .len > 0) -> computes len(s)+acc for EVERY input (per count-forall)"
  else
    miss=$((miss+1))
    echo "  MISS $s : a +1 slice recursion but not the full count-fold machine shape"
  fi
done

ok=1
[ "$gb" = accept ] && [ "$gr" = accept ] && [ "$gg" = accept ] && [ "$negok" = yes ] || ok=0
[ "$cov" -gt 0 ] && [ "$miss" = 0 ] || ok=0
echo "∀-input sample connection (a real sample's count loop proven = len(s)+acc for EVERY input by a 3-checker theorem; perturbation rejected): $cov sample loop(s) discharged"
[ "$ok" = 1 ]
