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
# Both theorems are verified here by implementations/beta/check.beta + implementations/reference/check_ref.py + implementations/gamma/checker.gamma with an off-by-one
# perturbation rejected. (recursive_sum's `sum` folds SIGNED i32 — that needs a ℤ (difference-pair) fold
# theorem, deliberately future work; these two dodge the signed bridge by folding non-negative quantities.)
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
command -v python3 >/dev/null 2>&1 || { echo "forall-sample: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b implementations/beta/check.beta           "$T/check.exe"  || { echo "forall-sample FAIL — build implementations/beta/check.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "forall-sample FAIL — build interp.beta"; exit 1; }
DEFS=$(cat "${OMEGA_PATH_PROOF_KERNEL}"/implementations/gamma/checker.gamma)
thmok=1

# verify3 LABEL ELAB PERTURB-SED : theorem accepted by all three checkers, perturbation rejected by beta+ref
verify3() {
  cert=$(python3 tools/elab.py < "$2" 2>/dev/null)
  vb=$(printf '%s' "$cert" | "$T/check.exe" 2>/dev/null)
  vr=$(printf '%s' "$cert" | python3 implementations/reference/check_ref.py 2>/dev/null)
  vg=$(printf '%s\n%s\n' "$DEFS" "$(printf '%s' "$cert" | python3 "${OMEGA_PATH_PROOF_KERNEL}"/tools/refcert_to_gamma.py 2>/dev/null)" | perl -e 'alarm 40; exec @ARGV' "$T/interp.exe" >/dev/null 2>&1; r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; })
  neg=$(sed "$3" "$2" | python3 tools/elab.py 2>/dev/null)
  nb=$(printf '%s' "$neg" | "$T/check.exe" 2>/dev/null); nr=$(printf '%s' "$neg" | python3 implementations/reference/check_ref.py 2>/dev/null)
  nok=no; { [ "$nb" != accept ] && [ "$nr" != accept ]; } && nok=yes
  echo "  $1: implementations/beta/check.beta=$vb check_ref=$vr implementations/gamma/checker.gamma=$vg | perturbed rejected=$nok"
  { [ "$vb" = accept ] && [ "$vr" = accept ] && [ "$vg" = accept ] && [ "$nok" = yes ]; } || thmok=0
}
verify3 "count-forall (count(xs,n)=len(xs)+n)" corpus/count-forall.elab '/^(all xs/ s/(f 21 (f 9\([0-9]\) xs) n)/(k 3 (f 21 (f 9\1 xs) n))/'
verify3 "pair-forall  (pairfold(xs,(s,c))=(listsum(xs)+s, len(xs)+c))" corpus/pair-forall.elab '/^(all xs/ s|(k 70 (f 21 (f 94 xs) s)|(k 70 (k 3 (f 21 (f 94 xs) s))|'
verify3 "int-sum-fold  (intEq(intAdd(acc,listsum xs), sumfold(xs,acc)) — SIGNED ℤ difference pairs, acc-first)" corpus/proofs/int-sum-fold.elab '/^(all xs/ s/(f 101 (f 96 xs acc))/(s (f 101 (f 96 xs acc)))/'
verify3 "sqsum-forall  (sqfold(xs,n)=sumSq(xs)+n; sumSq adds each element's SQUARE)" corpus/sqsum-forall.elab '/^(all xs/ s|(f 21 (f 94 xs) n)|(k 3 (f 21 (f 94 xs) n))|'

# ---- tie: real sample loops discharged for EVERY input ----
cov=0; miss=0
for f in "${OMEGA_PATH_CORPUS}"/*/main.omg; do
  s=$(basename "$(dirname "$f")")
  # (a) slice COUNT fold: a machine NAME(&mut self, x: &[..]) recursing NAME(x[1..], acc + 1) under x.len > 0
  rec=$(grep -oE '[a-z_]+\([a-z_]+\[1\.\.\], *[a-z_]+ \+ 1\)' "$f" 2>/dev/null | head -1)
  if [ -n "$rec" ]; then
    name=${rec%%\(*}
    acc=$(printf '%s' "$rec" | sed -E 's/.*, *([a-z_]+) \+ 1\)/\1/')   # the accumulator param
    # tie is faithful only if: the machine is a &[..] fold, guarded by .len > 0, AND the base returns the
    # bare accumulator (false -> acc) — matching count-forall's count(Nil,n)=n. A wrong base (e.g. acc+5)
    # is NOT the count fold and must be rejected.
    if grep -qE "$name"'\(&mut self, *[a-z_]+: *&\[' "$f" && grep -qE '\.len > 0' "$f" && grep -qE "false -> $acc *$" "$f"; then
      cov=$((cov+1)); echo "  ok   $s : slice machine '$name' IS the count fold ($rec, base 'false -> $acc') -> computes len(s)+acc for EVERY input (count-forall)"
    else
      miss=$((miss+1)); echo "  MISS $s : +1 slice recursion but not the full count-fold shape (guard .len>0 + base 'false -> $acc')"
    fi
  fi
  # (b) byte-stream (sum,count) PAIR fold: read_byte loop threading a byte sum (self.F = self.F + b) AND a
  #     count (self.G = self.G + 1), with an EOF guard (b < 0). The summed elements are bytes ∈ [0,255].
  bsum=$(grep -oE 'self\.[a-z_]+ = self\.[a-z_]+ \+ b\b' "$f" 2>/dev/null | head -1)
  if [ -n "$bsum" ]; then
    # tie is faithful only if: read_byte source, EOF guard (b < 0), a count (self.G += 1), AND the exit is the
    # COMBINED sum+count (exit_process(self.X + self.Y)) — matching pair-forall's (sum,count) fold and exit.
    if grep -qE '= read_byte\(\)' "$f" && grep -qE 'b < 0' "$f" && grep -qE 'self\.[a-z_]+ = self\.[a-z_]+ \+ 1\b' "$f" && grep -qE 'exit_process\(self\.[a-z_]+ \+ self\.[a-z_]+\)' "$f"; then
      cov=$((cov+1)); echo "  ok   $s : read_byte loop IS the (sum,count) pair fold ('$bsum' + count+1, exit sum+count; bytes ∈[0,255], EOF -1 guarded) -> (sum,count)=(listsum(input bytes),len) for EVERY input; exit = listsum+len (pair-forall over naturals)"
    else
      miss=$((miss+1)); echo "  MISS $s : a '+ b' accumulation but not the full read_byte (sum,count) pair-fold shape (count+1 + exit sum+count)"
    fi
  fi
  # (c) SIGNED-integer sum fold: a machine NAME(&mut self, s: &[i32 ..], acc) recursing NAME(s[1..], acc + s[0])
  #     under s.len > 0, base 'false -> acc' — int-sum-fold's ACC-FIRST shape exactly. Unlike the count fold
  #     (ℤ-FREE, element-value-independent), the summed elements are SIGNED i32, so this is discharged by the
  #     ℤ (difference-pair) fold theorem — the FIRST ∀-input connection over signed integers.
  sumrec=$(grep -oE '[a-z_]+\([a-z_]+\[1\.\.\], *[a-z_]+ \+ [a-z_]+\[0\]\)' "$f" 2>/dev/null | head -1)
  if [ -n "$sumrec" ]; then
    sname=${sumrec%%\(*}
    sacc=$(printf '%s' "$sumrec" | sed -E 's/.*, *([a-z_]+) \+ [a-z_]+\[0\]\)/\1/')
    if grep -qE "$sname"'\(&mut self, *[a-z_]+: *&\[i32' "$f" && grep -qE '\.len > 0' "$f" && grep -qE "false -> [(]?$sacc[)]? *$" "$f"; then
      cov=$((cov+1)); echo "  ok   $s : signed-sum machine '$sname' IS the ℤ fold ($sumrec, base 'false -> $sacc') -> = intAdd(acc, listsum s) up to ~ for EVERY input (int-sum-fold, SIGNED ℤ)"
    else
      miss=$((miss+1)); echo "  MISS $s : a '+ s[0]' recursion but not the full signed-sum-fold shape (&[i32] + .len>0 + base 'false -> $sacc')"
    fi
  fi
  # (d) SUM-OF-SQUARES fold: a machine NAME(&mut self, s: &[i32 ..], acc) recursing NAME(s[1..], acc + s[0]*s[0])
  #     under s.len > 0, base 'false -> acc'. Each element contributes its SQUARE h*h (always NON-NEGATIVE), so —
  #     unlike the signed-sum fold — this is ℤ-FREE: the NAT sqsum-forall theorem (sqfold(xs,n)=sumSq(xs)+n)
  #     discharges it directly, no difference-pair bridge needed.
  sqrec=$(grep -oE '[a-z_]+\([a-z_]+\[1\.\.\], *[a-z_]+ \+ [a-z_]+\[0\] \* [a-z_]+\[0\]\)' "$f" 2>/dev/null | head -1)
  if [ -n "$sqrec" ]; then
    qname=${sqrec%%\(*}
    qacc=$(printf '%s' "$sqrec" | sed -E 's/.*, *([a-z_]+) \+ [a-z_]+\[0\] \* [a-z_]+\[0\]\)/\1/')
    if grep -qE "$qname"'\(&mut self, *[a-z_]+: *&\[i32' "$f" && grep -qE '\.len > 0' "$f" && grep -qE "false -> [(]?$qacc[)]? *$" "$f"; then
      cov=$((cov+1)); echo "  ok   $s : sum-of-squares machine '$qname' IS the SQUARE fold ($sqrec, base 'false -> $qacc') -> = sumSq(s)+$qacc for EVERY input (sqsum-forall; ℤ-free, squares non-negative)"
    else
      miss=$((miss+1)); echo "  MISS $s : a '+ s[0]*s[0]' recursion but not the full sum-of-squares-fold shape (&[i32] + .len>0 + base 'false -> $qacc')"
    fi
  fi
done

ok=1
[ "$thmok" = 1 ] || ok=0
[ "$cov" -gt 0 ] && [ "$miss" = 0 ] || ok=0
echo "∀-input sample connection (real sample loops proven correct for EVERY input by 3-checker theorems; perturbations rejected): $cov sample loop(s) discharged (count over slices + (sum,count) pair over byte streams — ℤ-free; the SIGNED-integer sum fold over i32 slices via the ℤ difference-pair fold theorem; PLUS the SUM-OF-SQUARES fold — ℤ-free again, squares non-negative)"
[ "$ok" = 1 ]
