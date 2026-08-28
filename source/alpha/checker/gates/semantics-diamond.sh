#!/usr/bin/env sh
# SEMANTICS DIAMOND — the proof/meaning soundness seam, as a cross-check.
#
# proof_kernel.md's honest-edge #1: a proof must connect to "what a program actually does
# per the reference interpreter" — a soundness theorem at the proof/meaning seam. We
# cannot prove that theorem here, but we can EXHIBIT the seam: the checker's
# DEFINITIONAL equality (implementations/beta/eq.beta: reduce both sides to normal form) must agree with
# the interpreter's OPERATIONAL evaluation (interp.beta running gamma's own `plus`)
# for every Peano equation. Two different routes to "is e1 = e2" — definitional vs
# operational — agreeing is evidence the checker's equality is sound w.r.t. the
# reference interpreter (not a proof; the theorem is the open problem).
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_ALPHA_CHECKER"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b implementations/beta/eq.beta "$T/eq.exe"           || { echo "build implementations/beta/eq.beta failed"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
# gamma operational defs: plus/mult + neq (Nat eq); append/length + leq (List eq);
# g embeds a user-Nat (Z/S constructors) into Peano — the operational twin of implementations/beta/eq.beta's
# (fun 7 …) rules, so user-FUNCTION reduction is cross-checked against the interpreter.
DEFS='(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) (def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) (def neq (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (neq x y)) (w 0))))) (def append (a b) (match a (Lnil b) ((Lcons h t) (Lcons h (append t b))))) (def length (l) (match l (Lnil Ze) ((Lcons h t) (Su (length t))))) (def leq (a b) (match a (Lnil (match b (Lnil 1) (w 0))) ((Lcons h t) (match b ((Lcons i u) (if (neq h i) (leq t u) 0)) (w 0))))) (def g (n) (match n (Z Ze) ((S x) (Su (g x)))))'

PASS=0; FAIL=0
# dia DESC  EQ_BETA_INPUT(p/s/z)  GAMMA_EXPR(Su/Ze + plus)  EXPECT(equal|differ)
dia() {
  veq=$(printf '%s' "$2" | "$T/eq.exe")                              # implementations/beta/eq.beta: equal/differ
  printf '%s\n(neq %s)\n' "$DEFS" "$3" | "$T/interp.exe" >/dev/null; n=$?  # interp: 1/0
  vop=differ; [ "$n" = 1 ] && vop=equal
  if [ "$veq" = "$vop" ] && [ "$veq" = "$4" ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $1 : definitional=$veq operational=$vop expect=$4"; fi
}
# dial — same, for LIST-valued results (compared with leq instead of neq)
dial() {
  veq=$(printf '%s' "$2" | "$T/eq.exe")
  printf '%s\n(leq %s)\n' "$DEFS" "$3" | "$T/interp.exe" >/dev/null; n=$?
  vop=differ; [ "$n" = 1 ] && vop=equal
  if [ "$veq" = "$vop" ] && [ "$veq" = "$4" ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $1 : definitional=$veq operational=$vop expect=$4"; fi
}
#    desc        implementations/beta/eq.beta (definitional)                                gamma (operational, plus)                                          expect
dia "2+2 = 4"    "(p (s (s z)) (s (s z)))  (s (s (s (s z))))"          "(plus (Su (Su Ze)) (Su (Su Ze))) (Su (Su (Su (Su Ze))))"           equal
dia "0+3 = 3"    "(p z (s (s (s z))))  (s (s (s z)))"                  "(plus Ze (Su (Su (Su Ze)))) (Su (Su (Su Ze)))"                     equal
dia "3+0 = 3"    "(p (s (s (s z))) z)  (s (s (s z)))"                  "(plus (Su (Su (Su Ze))) Ze) (Su (Su (Su Ze)))"                     equal
dia "1+2 = 3"    "(p (s z) (s (s z)))  (s (s (s z)))"                  "(plus (Su Ze) (Su (Su Ze))) (Su (Su (Su Ze)))"                     equal
dia "2+2 != 5"   "(p (s (s z)) (s (s z)))  (s (s (s (s (s z)))))"      "(plus (Su (Su Ze)) (Su (Su Ze))) (Su (Su (Su (Su (Su Ze)))))"      differ
dia "1+1 != 1"   "(p (s z) (s z))  (s z)"                              "(plus (Su Ze) (Su Ze)) (Su Ze)"                                   differ
dia "2*3 = 6"    "(m (s (s z)) (s (s (s z))))  (s (s (s (s (s (s z))))))" "(mult (Su (Su Ze)) (Su (Su (Su Ze)))) (Su (Su (Su (Su (Su (Su Ze))))))" equal
dia "3*0 = 0"    "(m (s (s (s z))) z)  z"                              "(mult (Su (Su (Su Ze))) Ze) Ze"                                   equal
dia "2*3 != 5"   "(m (s (s z)) (s (s (s z))))  (s (s (s (s (s z)))))"  "(mult (Su (Su Ze)) (Su (Su (Su Ze)))) (Su (Su (Su (Su (Su Ze)))))" differ
# Lists: append/length, definitional (implementations/beta/eq.beta) vs operational (interpreter)
dia "len[_,_]=2"   "(len (cons z (cons z nil)))  (s (s z))"                "(length (Lcons Ze (Lcons Ze Lnil))) (Su (Su Ze))"                  equal
dia "len(a++b)=2"  "(len (app (cons z nil) (cons z nil)))  (s (s z))"      "(length (append (Lcons Ze Lnil) (Lcons Ze Lnil))) (Su (Su Ze))"    equal
# user-FUNCTION reduction: implementations/beta/eq.beta's (fun 7 …) rules vs interp's operational (def g …)
dia "fun g(S Z)=1"  "(fun 7 2 z) (fun 7 3 (s (rec 0))) (f 7 (k 3 (k 2)))  (s z)"            "(g (S Z)) (Su Ze)"            equal
dia "fun g(SSZ)=2"  "(fun 7 2 z) (fun 7 3 (s (rec 0))) (f 7 (k 3 (k 3 (k 2))))  (s (s z))"  "(g (S (S Z))) (Su (Su Ze))"   equal
dia "fun g(S Z)!=2" "(fun 7 2 z) (fun 7 3 (s (rec 0))) (f 7 (k 3 (k 2)))  (s (s z))"          "(g (S Z)) (Su (Su Ze))"       differ
# BINARY user-function: implementations/beta/eq.beta's user-add(x,y) over user-Nat reduces to the same value
# as interp's built-in plus — a value-level cross-check of multi-argument reduction.
dia "fun add(1,1)=2" "(fun 10 2 (y 0)) (fun 10 3 (k 3 (rec 0))) (f 10 (k 3 (k 2)) (k 3 (k 2)))  (k 3 (k 3 (k 2)))" "(plus (Su Ze) (Su Ze)) (Su (Su Ze))" equal
dia "fun add(2,1)=3" "(fun 10 2 (y 0)) (fun 10 3 (k 3 (rec 0))) (f 10 (k 3 (k 3 (k 2))) (k 3 (k 2)))  (k 3 (k 3 (k 3 (k 2))))" "(plus (Su (Su Ze)) (Su Ze)) (Su (Su (Su Ze)))" equal
# COMPOSED user-function: implementations/beta/eq.beta's user-mult (fid 11, whose rule body CALLS user-add fid 10)
# vs interp's operational mult — a value-level cross-check of function-calling-function.
MFUN="(fun 10 2 (y 0)) (fun 10 3 (k 3 (rec 0))) (fun 11 2 (k 2)) (fun 11 3 (f 10 (y 0) (rec 0)))"
dia "fun mult(2,3)=6" "$MFUN (f 11 (k 3 (k 3 (k 2))) (k 3 (k 3 (k 3 (k 2)))))  (k 3 (k 3 (k 3 (k 3 (k 3 (k 3 (k 2)))))))" "(mult (Su (Su Ze)) (Su (Su (Su Ze)))) (Su (Su (Su (Su (Su (Su Ze))))))" equal
dia "fun mult(3,0)=0" "$MFUN (f 11 (k 3 (k 3 (k 3 (k 2)))) (k 2))  (k 2)" "(mult (Su (Su (Su Ze))) Ze) Ze" equal
dia "fun mult(2,3)!=5" "$MFUN (f 11 (k 3 (k 3 (k 2))) (k 3 (k 3 (k 3 (k 2)))))  (k 3 (k 3 (k 3 (k 3 (k 3 (k 2))))))" "(mult (Su (Su Ze)) (Su (Su (Su Ze)))) (Su (Su (Su (Su (Su Ze)))))" differ
dial "[0]++[1]"    "(app (cons z nil) (cons (s z) nil))  (cons z (cons (s z) nil))" "(append (Lcons Ze Lnil) (Lcons (Su Ze) Lnil)) (Lcons Ze (Lcons (Su Ze) Lnil))" equal
dial "[]++[0]"     "(app nil (cons z nil))  (cons z nil)"                  "(append Lnil (Lcons Ze Lnil)) (Lcons Ze Lnil)"                     equal
dial "[0] != []"   "(app (cons z nil) nil)  nil"                          "(append (Lcons Ze Lnil) Lnil) Lnil"                               differ
echo "semantics diamond (definitional eq vs operational eval): $PASS agree, $FAIL disagree"
[ "$FAIL" = 0 ] || exit 1
