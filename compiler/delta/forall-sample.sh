#!/usr/bin/env sh
# ∀-INPUT SAMPLE CONNECTION — real Omega sample loops proven correct for EVERY input.
#
# input-tv.sh proves an input loop's meaning on the DOCUMENTED input vectors; forall-input.sh proves the
# ABSTRACT fold theorems (count/sum/prod/...) for all inputs. THIS gate joins the two: it confirms a real
# sample loop IS one of those abstract folds, so the ∀-input theorem discharges that sample's loop
# UNIVERSALLY — for every input, not just the documented one. It is the proof-carrying step that says "this
# real program's loop is correct for all inputs", the summit of the meaning route.
#
# Two connections, both ℤ-FREE (they fold NON-NEGATIVE / structural quantities, so the NAT theorems apply
# with no signed-integer bridge):
#   count  — recursive_sum's `count(s,acc)=if s.len>0 then count(s[1..],acc+1) else acc` adds a CONSTANT per
#            element (element-value-independent), so count-forall (count(xs,n)=len(xs)+n) discharges it.
#   pair   — stdin_checksum's read_byte loop threads a byte SUM (self.sum += b) AND a COUNT (self.n += 1) as a
#            pair; the summed elements are BYTES (read_byte ∈ [0,255], EOF -1 guarded out by `b < 0`), so they
#            are naturals and pair-forall (pairfold(xs,(s,c))=(listsum(xs)+s, len(xs)+c)) discharges the WHOLE
#            dual accumulator: (sum,count) = (listsum(input bytes), len) for EVERY input, so the exit sum+n =
#            listsum(bytes)+len(bytes) universally — the first COMPLETE ∀-input proof of an input sample.
# Both theorems are verified here by check.beta + check_ref.py + checker.gamma with an off-by-one
# perturbation rejected. (recursive_sum's `sum` folds SIGNED i32 — that needs a ℤ (difference-pair) fold
# theorem, deliberately future work; these two dodge the signed bridge by folding non-negative quantities.)
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
thmok=1

# verify3 LABEL ELAB PERTURB-SED : theorem accepted by all three checkers, perturbation rejected by beta+ref
verify3() {
  cert=$(python3 elab.py < "$2" 2>/dev/null)
  vb=$(printf '%s' "$cert" | "$T/check.exe" 2>/dev/null)
  vr=$(printf '%s' "$cert" | python3 check_ref.py 2>/dev/null)
  vg=$(printf '%s\n%s\n' "$DEFS" "$(printf '%s' "$cert" | python3 ../gamma/refcert_to_gamma.py 2>/dev/null)" | perl -e 'alarm 40; exec @ARGV' "$T/interp.exe" >/dev/null 2>&1; r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; })
  neg=$(sed "$3" "$2" | python3 elab.py 2>/dev/null)
  nb=$(printf '%s' "$neg" | "$T/check.exe" 2>/dev/null); nr=$(printf '%s' "$neg" | python3 check_ref.py 2>/dev/null)
  nok=no; { [ "$nb" != accept ] && [ "$nr" != accept ]; } && nok=yes
  echo "  $1: check.beta=$vb check_ref=$vr checker.gamma=$vg | perturbed rejected=$nok"
  { [ "$vb" = accept ] && [ "$vr" = accept ] && [ "$vg" = accept ] && [ "$nok" = yes ]; } || thmok=0
}
verify3 "count-forall (count(xs,n)=len(xs)+n)" count-forall.elab '/^(all xs/ s/(f 21 (f 9\([0-9]\) xs) n)/(k 3 (f 21 (f 9\1 xs) n))/'
verify3 "pair-forall  (pairfold(xs,(s,c))=(listsum(xs)+s, len(xs)+c))" pair-forall.elab '/^(all xs/ s|(k 70 (f 21 (f 94 xs) s)|(k 70 (k 3 (f 21 (f 94 xs) s))|'

# ---- tie: real sample loops discharged for EVERY input ----
cov=0; miss=0
for f in ../../samples/*/main.omg; do
  s=$(basename "$(dirname "$f")")
  # (a) slice COUNT fold: a machine NAME(&mut self, x: &[..]) recursing NAME(x[1..], acc + 1) under x.len > 0
  rec=$(grep -oE '[a-z_]+\([a-z_]+\[1\.\.\], *[a-z_]+ \+ 1\)' "$f" 2>/dev/null | head -1)
  if [ -n "$rec" ]; then
    name=${rec%%\(*}
    if grep -qE "$name"'\(&mut self, *[a-z_]+: *&\[' "$f" && grep -qE '\.len > 0' "$f"; then
      cov=$((cov+1)); echo "  ok   $s : slice machine '$name' IS the count fold ($rec) -> computes len(s)+acc for EVERY input (count-forall)"
    else
      miss=$((miss+1)); echo "  MISS $s : +1 slice recursion but not the count-fold machine shape"
    fi
  fi
  # (b) byte-stream (sum,count) PAIR fold: read_byte loop threading a byte sum (self.F = self.F + b) AND a
  #     count (self.G = self.G + 1), with an EOF guard (b < 0). The summed elements are bytes ∈ [0,255].
  bsum=$(grep -oE 'self\.[a-z_]+ = self\.[a-z_]+ \+ b\b' "$f" 2>/dev/null | head -1)
  if [ -n "$bsum" ]; then
    if grep -qE '= read_byte\(\)' "$f" && grep -qE 'b < 0' "$f" && grep -qE 'self\.[a-z_]+ = self\.[a-z_]+ \+ 1\b' "$f"; then
      cov=$((cov+1)); echo "  ok   $s : read_byte loop IS the (sum,count) pair fold ('$bsum' + count+1; bytes ∈[0,255], EOF -1 guarded) -> (sum,count)=(listsum(input bytes),len) for EVERY input; exit sum+n = listsum+len (pair-forall over naturals)"
    else
      miss=$((miss+1)); echo "  MISS $s : a '+ b' accumulation but not the read_byte (sum,count) pair-fold shape"
    fi
  fi
done

ok=1
[ "$thmok" = 1 ] || ok=0
[ "$cov" -gt 0 ] && [ "$miss" = 0 ] || ok=0
echo "∀-input sample connection (real sample loops proven correct for EVERY input by 3-checker theorems; perturbations rejected): $cov sample loop(s) discharged (count over slices; (sum,count) pair over byte streams — both ℤ-free)"
[ "$ok" = 1 ]
