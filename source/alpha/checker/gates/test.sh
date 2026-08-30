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
cd "$OMEGA_PATH_ALPHA_CHECKER"

TMP=$(mktemp -d)
trap 'trash "$TMP"' EXIT
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
chk "raw subject constants require a frame" "(= source source) (refl source)" reject

# The published profile fails closed at both its outer input boundary and its
# permanent-arena boundary. Neither exhaustion may trap or accidentally accept.
dd if=/dev/zero of="$TMP/input-over" bs=2024317 count=1 2>/dev/null
file_chk "complete input extent" "$TMP/input-over" 0 reject

arena_cert='(= z z) (refl z)'
: > "$TMP/arena-over"
printf 'OMGCHK1\n' >> "$TMP/arena-over"
append_u64le 262144 "$TMP/arena-over"
dd if=/dev/zero bs=262144 count=1 2>/dev/null >> "$TMP/arena-over"
append_u64le 262140 "$TMP/arena-over"
dd if=/dev/zero bs=262140 count=1 2>/dev/null >> "$TMP/arena-over"
append_u64le "$(printf '%s' "$arena_cert" | wc -c | tr -d ' ')" "$TMP/arena-over"
printf '%s' "$arena_cert" >> "$TMP/arena-over"
file_chk "permanent arena extent" "$TMP/arena-over" 0 reject

echo "checker rule discriminators: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
