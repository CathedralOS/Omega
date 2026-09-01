#!/usr/bin/env sh
# Compact discriminator suite for the authoritative Alpha-tape checker.
# This is rule coverage, not a theorem library: each retained case distinguishes
# an implemented calculus boundary. Broad differential coverage lives in the
# single independent reference diamond.
set -u

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh"
. "$OMEGA_PATH_GAMMA_COMPILER/artifact_env.sh"
cd "$OMEGA_PATH_ALPHA_CHECKER"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT
stamp_proof_checker "$TMP/check" >/dev/null

PASS=0
FAIL=0
chk() { # description certificate expected-outcome
  out=$(printf '%s' "$2" | "$TMP/check")
  if [ "$out" = "$3" ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "FAIL: $1 — expected $3, got '$out'"
  fi
}

append_u64le() { # value file
  frame_value=$1
  frame_file=$2
  frame_i=0
  while [ "$frame_i" -lt 8 ]; do
    frame_byte=$((frame_value % 256))
    frame_octal=$(printf '%03o' "$frame_byte")
    printf "\\$frame_octal" >> "$frame_file"
    frame_value=$((frame_value / 256))
    frame_i=$((frame_i + 1))
  done
}

frame_chk() { # description source-bytes tape-bytes certificate expected-outcome
  frame_path="$TMP/frame"
  : > "$frame_path"
  printf 'OMGCHK1\n' >> "$frame_path"
  frame_source_len=$(printf '%s' "$2" | wc -c | tr -d ' ')
  append_u64le "$frame_source_len" "$frame_path"
  printf '%s' "$2" >> "$frame_path"
  frame_tape_len=$(printf '%s' "$3" | wc -c | tr -d ' ')
  append_u64le "$frame_tape_len" "$frame_path"
  printf '%s' "$3" >> "$frame_path"
  frame_cert_len=$(printf '%s' "$4" | wc -c | tr -d ' ')
  append_u64le "$frame_cert_len" "$frame_path"
  printf '%s' "$4" >> "$frame_path"
  out=$("$TMP/check" < "$frame_path")
  if [ "$out" = "$5" ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "FAIL: $1 — expected $5, got '$out'"
  fi
}

write_frame_files() { # output source-file tape-file certificate-file
  frame_output=$1
  frame_source=$2
  frame_tape=$3
  frame_certificate=$4
  : > "$frame_output"
  printf 'OMGCHK1\n' >> "$frame_output"
  append_u64le "$(wc -c < "$frame_source" | tr -d ' ')" "$frame_output"
  dd if="$frame_source" status=none >> "$frame_output"
  append_u64le "$(wc -c < "$frame_tape" | tr -d ' ')" "$frame_output"
  dd if="$frame_tape" status=none >> "$frame_output"
  append_u64le "$(wc -c < "$frame_certificate" | tr -d ' ')" "$frame_output"
  dd if="$frame_certificate" status=none >> "$frame_output"
}

space_file() { # extent output
  dd if=/dev/zero bs="$1" count=1 2>/dev/null | tr '\000' ' ' > "$2"
}

file_chk() { # description input-file expected-status expected-output
  set +e
  file_out=$("$TMP/check" < "$2")
  file_status=$?
  set -e
  if [ "$file_status" = "$3" ] && [ "$file_out" = "$4" ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "FAIL: $1 — expected $3/$4, got $file_status/'$file_out'"
  fi
}

# Propositional natural deduction and resource boundaries.
chk "implication introduction" "(-> P P) (lam P (hyp 0))" accept
chk "unbound hypothesis" "P (hyp 0)" reject
chk "implication elimination" "(-> (& (-> P Q) P) Q) (lam (& (-> P Q) P) (app (fst (hyp 0)) (snd (hyp 0))))" accept
chk "pair projection mismatch" "(-> (& P Q) Q) (lam (& P Q) (fst (hyp 0)))" reject
chk "sum elimination" "(-> (+ P Q) (+ Q P)) (lam (+ P Q) (case (hyp 0) (lam P (inr Q (hyp 0))) (lam Q (inl P (hyp 0)))))" accept
chk "sum branches disagree" "(-> (+ P Q) P) (lam (+ P Q) (case (hyp 0) (lam P (hyp 0)) (lam Q (hyp 0))))" reject
chk "falsity elimination" "(-> (bot) P) (lam (bot) (absurd P (hyp 0)))" accept
chk "falsity requires bottom" "(-> P Q) (lam P (absurd Q (hyp 0)))" reject

# Definitional equality and first-order binding.
chk "computed reflexivity" "(= (p (s z) (s z)) (s (s z))) (refl (s (s z)))" accept
chk "false computation" "(= (p (s z) (s z)) (s z)) (refl (s z))" reject
chk "universal introduction" "(All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (gen (lam (Pred 0 (v 0)) (hyp 0)))" accept
chk "universal instantiation" "(-> (All (Pred 0 (v 0))) (Pred 0 z)) (lam (All (Pred 0 (v 0))) (inst (hyp 0) z))" accept
chk "generalization capture" "(-> (Pred 0 z) (All (Pred 0 (v 0)))) (lam (Pred 0 z) (gen (hyp 0)))" reject
chk "existential introduction" "(-> (Pred 0 z) (Exists (Pred 0 (v 0)))) (lam (Pred 0 z) (wit (Pred 0 (v 0)) z (hyp 0)))" accept
chk "existential witness mismatch" "(-> (Pred 0 z) (Exists (Pred 0 (v 0)))) (lam (Pred 0 z) (wit (Pred 0 (v 0)) (s z) (hyp 0)))" reject
chk "relation argument order" "(-> (Rel 0 z (s z)) (Rel 0 (s z) z)) (lam (Rel 0 z (s z)) (hyp 0))" reject

# D40's closed FloatMeaning correspondence term has one carrier-specific
# equality. The key binds format, projection operation, exact core declaration,
# numeric catalog, and a verifier-reconstructed source coordinate. It is proof
# metadata only: ordinary equality and generic relations cannot consume it.
FM32_NAN='(fm 32 1 1 1 4 2143289345 0)'
FM32_POS_ZERO='(fm 32 1 1 1 4 0 0)'
FM32_NEG_ZERO='(fm 32 1 1 1 4 2147483648 0)'
FM64_NAN='(fm 64 2 2 1 4 0 2146959360)'
FM32_TERMINAL='(fm 32 1 1 1 2 17 0)'
chk "FloatMeaning binary32 NaN reflexivity" "(FloatMeaningEqual $FM32_NAN $FM32_NAN) (fmrefl $FM32_NAN)" accept
chk "FloatMeaning binary64 NaN reflexivity" "(FloatMeaningEqual $FM64_NAN $FM64_NAN) (fmrefl $FM64_NAN)" accept
chk "FloatMeaning quantifier substitution preserves closed identity" "(FloatMeaningEqual $FM32_TERMINAL $FM32_TERMINAL) (inst (gen (fmrefl $FM32_TERMINAL)) z)" accept
chk "FloatMeaning signed zero does not coalesce" "(FloatMeaningEqual $FM32_POS_ZERO $FM32_NEG_ZERO) (fmrefl $FM32_POS_ZERO)" reject
chk "FloatMeaning distinct terms require an explicit theorem" "(-> (FloatMeaningEqual $FM32_POS_ZERO $FM32_NEG_ZERO) (FloatMeaningEqual $FM32_POS_ZERO $FM32_NEG_ZERO)) (lam (FloatMeaningEqual $FM32_POS_ZERO $FM32_NEG_ZERO) (hyp 0))" accept
chk "FloatMeaning source coordinate mutation does not coalesce" "(FloatMeaningEqual $FM32_TERMINAL (fm 32 1 1 1 2 18 0)) (fmrefl $FM32_TERMINAL)" reject
chk "FloatMeaning lookalike core declaration" "(FloatMeaningEqual (fm 32 1 9 1 4 0 0) (fm 32 1 9 1 4 0 0)) (fmrefl (fm 32 1 9 1 4 0 0))" reject
chk "FloatMeaning cross-format projection substitution" "(FloatMeaningEqual (fm 32 2 1 1 4 0 0) (fm 32 2 1 1 4 0 0)) (fmrefl (fm 32 2 1 1 4 0 0))" reject
chk "FloatMeaning catalog substitution" "(FloatMeaningEqual (fm 32 1 1 2 4 0 0) (fm 32 1 1 2 4 0 0)) (fmrefl (fm 32 1 1 2 4 0 0))" reject
chk "FloatMeaning noncanonical Terminal source" "(FloatMeaningEqual (fm 32 1 1 1 2 17 1) (fm 32 1 1 1 2 17 1)) (fmrefl (fm 32 1 1 1 2 17 1))" reject
chk "FloatMeaning cannot use ordinary equality" "(= $FM32_NAN $FM32_NAN) (refl $FM32_NAN)" reject
chk "FloatMeaning cannot become an IEEE-like generic relation" "(Rel 900 $FM32_NAN $FM32_NAN) (fmrefl $FM32_NAN)" reject
chk "FloatMeaning proposition lookalike spelling" "(FloatMeaningAlias $FM32_NAN $FM32_NAN) (fmrefl $FM32_NAN)" reject
chk "FloatMeaning term lookalike spelling" "(FloatMeaningEqual (fmalias 32 1 1 1 4 2143289345 0) (fmalias 32 1 1 1 4 2143289345 0)) (fmrefl (fmalias 32 1 1 1 4 2143289345 0))" reject
chk "FloatMeaning proof lookalike spelling" "(FloatMeaningEqual $FM32_NAN $FM32_NAN) (fmreflalias $FM32_NAN)" reject

# Induction, equality transport, and no-confusion.
chk "natural induction" "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "natural induction needs successor step" "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject
chk "equality transport" "(-> (Pred 0 (p (s z) (s z))) (Pred 0 (s (s z)))) (lam (Pred 0 (p (s z) (s z))) (eqelim (Pred 0 (v 0)) (refl (s (s z))) (hyp 0)))" accept
chk "transport mismatch" "(-> (Pred 0 (s z)) (Pred 0 (s (s z)))) (lam (Pred 0 (s z)) (eqelim (Pred 0 (v 0)) (refl (s (s z))) (hyp 0)))" reject
chk "zero/successor disjoint" "(-> (= z (s z)) (bot)) (lam (= z (s z)) (disj (hyp 0)))" accept
chk "successor injective" "(-> (= (s (v 0)) (s z)) (= (v 0) z)) (lam (= (s (v 0)) (s z)) (sinj (hyp 0)))" accept

# Lists and user-declared structural recursion.
chk "append computation" "(= (app (cons z nil) (cons (s z) nil)) (cons z (cons (s z) nil))) (refl (cons z (cons (s z) nil)))" accept
chk "list induction" "(-> (Pred 0 nil) (-> (All (All (-> (Pred 0 (v 0)) (Pred 0 (cons (v 1) (v 0)))))) (All (Pred 0 (v 0))))) (lam (Pred 0 nil) (lam (All (All (-> (Pred 0 (v 0)) (Pred 0 (cons (v 1) (v 0)))))) (listind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "constructor structure" "(= (k 1 (k 0) (k 0)) (k 1 (k 0) (k 0))) (refl (k 1 (k 0) (k 0)))" accept
chk "constructor mismatch" "(= (k 0) (k 1 (k 0) (k 0))) (refl (k 0))" reject
chk "declared structural induction" "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "structural induction needs every IH" "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject

# Ground user functions and checked named lemmas.
FG="(data 2 0 0 0) (data 3 1 1 0) (fun 7 2 z) (fun 7 3 (s (rec 0)))"
chk "user function reduction" "$FG (= (f 7 (k 3 (k 2))) (s z)) (refl (f 7 (k 3 (k 2))))" accept
chk "user function wrong result" "$FG (= (f 7 (k 3 (k 2))) (s (s z))) (refl (f 7 (k 3 (k 2))))" reject
chk "open user function remains stuck" "$FG (= (f 7 (v 0)) z) (refl (f 7 (v 0)))" reject
chk "checked lemma use" "(def 0 (-> P P) (lam P (hyp 0))) (-> P P) (use 0)" accept
chk "invalid lemma definition" "(def 0 (-> P Q) (lam P (hyp 0))) (-> P Q) (use 0)" reject

# The two retained inductive predicates: membership and product witnesses.
chk "membership head" "(Rel 777 (s z) (cons (s z) nil)) (memhead (s z) nil)" accept
chk "membership cannot fabricate nil" "(Rel 777 (s z) nil) (memhead (s z) nil)" reject
chk "product witness" "(Rel 778 (cons (s (s z)) nil) (m (s (s z)) (s z))) (pcons (s (s z)) (pnil))" accept
chk "product witness result mismatch" "(Rel 778 nil (s (s z))) (pnil)" reject
chk "product nil inversion" "(All (-> (Rel 778 nil (v 0)) (= (v 0) (s z)))) (gen (lam (Rel 778 nil (v 0)) (prodnilinv (hyp 0))))" accept

# Framed subjects are checker-bound raw byte trees. The same certificate must
# distinguish a one-byte subject mutation, while legacy input cannot name them.
frame_chk "framed raw subjects equal" 'abc' 'abc' '(= source tape) (refl source)' accept
frame_chk "framed raw subject mutation" 'abc' 'abd' '(= source tape) (refl source)' reject
RAW_TREE_FOLD='(fun 100 61 z) (fun 100 62 (s z)) (fun 100 63 (p (rec 0) (rec 1)))'
frame_chk "framed raw tree is computable" 'abc' 'x' "$RAW_TREE_FOLD (= (f 100 source) (s (s (s z)))) (refl (s (s (s z))))" accept
frame_chk "certificate raw lookalike still substitutes" 'abc' 'x' \
  '(= (k 63 z (k 61)) (k 63 z (k 61))) (eqelim (= (k 63 (v 0) (k 61)) (k 63 (v 0) (k 61))) (refl z) (refl (k 63 z (k 61))))' accept
chk "raw subject constants require a frame" "(= source source) (refl source)" reject

# Equality transport may retain a pointer to the immutable checker-owned raw
# interval. Recursively copying this maximum-size source subject would exhaust
# the arena; the accepted proof therefore pins the closed-term fast path itself.
dd if=/dev/zero bs=262144 count=1 2>/dev/null | tr '\000' ' ' > "$TMP/raw-subst-source"
printf 'x' > "$TMP/raw-subst-tape"
printf '%s' '(= source source) (eqelim (= source source) (refl z) (refl source))' \
  > "$TMP/raw-subst-cert"
printf 'OMGCHK1\n' > "$TMP/raw-subst-frame"
append_u64le 262144 "$TMP/raw-subst-frame"
dd if="$TMP/raw-subst-source" status=none >> "$TMP/raw-subst-frame"
append_u64le 1 "$TMP/raw-subst-frame"
dd if="$TMP/raw-subst-tape" status=none >> "$TMP/raw-subst-frame"
append_u64le "$(wc -c < "$TMP/raw-subst-cert" | tr -d ' ')" "$TMP/raw-subst-frame"
dd if="$TMP/raw-subst-cert" status=none >> "$TMP/raw-subst-frame"
file_chk "checker raw interval survives transport" "$TMP/raw-subst-frame" 1 accept

# AlphaBootstrapV2's maximum tape is exercised as an actual compiler edge, not
# as a zero-filled allocation. This source produces 12 bytes per fixed emitted
# byte plus the 192-byte runnable-program envelope.
stamp_gamma_compiler "$TMP/gamma-compiler" >/dev/null
awk 'BEGIN { printf "proc main() { emit(\""; for (i = 0; i < 87365; i++) printf "a"; print "\") return 1 + 1 }" }' \
  > "$TMP/v2-max-core.gamma"
v2_source_core_extent=$(wc -c < "$TMP/v2-max-core.gamma" | tr -d ' ')
dd if="$TMP/v2-max-core.gamma" status=none > "$TMP/v2-max.gamma"
space_file $((262144 - v2_source_core_extent)) "$TMP/v2-source-pad"
dd if="$TMP/v2-source-pad" status=none >> "$TMP/v2-max.gamma"
set +e
"$TMP/gamma-compiler" < "$TMP/v2-max.gamma" > "$TMP/v2-max.tape"
v2_compile_status=$?
set -e
v2_tape_extent=$(wc -c < "$TMP/v2-max.tape" | tr -d ' ')
if [ "$v2_compile_status" -eq 0 ] && [ "$v2_tape_extent" -eq 1048572 ]; then
  PASS=$((PASS + 1))
else
  FAIL=$((FAIL + 1))
  echo "FAIL: V2 exact compiler tape — expected 0/1048572, got $v2_compile_status/$v2_tape_extent"
fi

# `first_raw` follows the immutable tree's left spine. The two checked lemmas
# force bounded normalization over both real subjects, and the final conjunction
# retains and reuses them rather than accepting a bare reflexive maximum tape.
printf '%s' \
  '(fun 100 61 (k 61)) (fun 100 62 (v 0)) (fun 100 63 (rec 0)) (def 0 (= (f 100 source) (k 60 (k 7) (k 0))) (refl (k 60 (k 7) (k 0)))) (def 1 (= (f 100 tape) (k 60 (k 0) (k 1))) (refl (k 60 (k 0) (k 1)))) (& (= (f 100 source) (k 60 (k 7) (k 0))) (= (f 100 tape) (k 60 (k 0) (k 1)))) (pair (use 0) (use 1))' \
  > "$TMP/v2-real-cert-core"
v2_real_cert_core_extent=$(wc -c < "$TMP/v2-real-cert-core" | tr -d ' ')
space_file $((1500000 - v2_real_cert_core_extent)) "$TMP/v2-real.cert"
dd if="$TMP/v2-real-cert-core" status=none >> "$TMP/v2-real.cert"
write_frame_files "$TMP/v2-real.frame" "$TMP/v2-max.gamma" "$TMP/v2-max.tape" "$TMP/v2-real.cert"
file_chk "V2 realistic complete input maximum" "$TMP/v2-real.frame" 1 accept

# Each immediately adjacent declared extent and the next complete input byte
# rejects before publication. The payload bytes are present, so these are real
# boundary cases rather than truncation aliases.
dd if="$TMP/v2-max.gamma" status=none > "$TMP/v2-source-over"
printf ' ' >> "$TMP/v2-source-over"
printf 'x' > "$TMP/v2-one"
printf '%s' '(= z z) (refl z)' > "$TMP/v2-small-cert"
write_frame_files "$TMP/v2-source-over-frame" "$TMP/v2-source-over" "$TMP/v2-one" "$TMP/v2-small-cert"
file_chk "V2 source adjacent extent" "$TMP/v2-source-over-frame" 0 reject

dd if="$TMP/v2-max.tape" status=none > "$TMP/v2-tape-over"
printf '\000' >> "$TMP/v2-tape-over"
write_frame_files "$TMP/v2-tape-over-frame" "$TMP/v2-one" "$TMP/v2-tape-over" "$TMP/v2-small-cert"
file_chk "V2 tape adjacent extent" "$TMP/v2-tape-over-frame" 0 reject

dd if="$TMP/v2-real.cert" status=none > "$TMP/v2-cert-over"
printf ' ' >> "$TMP/v2-cert-over"
write_frame_files "$TMP/v2-cert-over-frame" "$TMP/v2-one" "$TMP/v2-one" "$TMP/v2-cert-over"
file_chk "V2 certificate adjacent extent" "$TMP/v2-cert-over-frame" 0 reject

dd if="$TMP/v2-real.frame" status=none > "$TMP/v2-input-over"
printf 'x' >> "$TMP/v2-input-over"
file_chk "V2 complete input adjacent extent" "$TMP/v2-input-over" 0 reject

# A compact structural identity rebuild over the maximum tape authors a fresh
# balanced result rather than preserving the immutable subject pointer. It
# crosses the permanent/conversion arena with logarithmic recursion depth;
# failure must remain ordinary checker rejection, never a stack fault, trap, or
# accidental acceptance.
printf '%s' \
  '(fun 201 61 (k 61)) (fun 201 62 (k 62 (v 0))) (fun 201 63 (k 63 (rec 0) (rec 1))) (= (f 201 tape) tape) (refl tape)' \
  > "$TMP/v2-arena-over-cert"
write_frame_files "$TMP/v2-arena-over-frame" "$TMP/v2-max.gamma" "$TMP/v2-max.tape" "$TMP/v2-arena-over-cert"
file_chk "V2 permanent arena exhaustion" "$TMP/v2-arena-over-frame" 0 reject

echo "checker rule discriminators: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
