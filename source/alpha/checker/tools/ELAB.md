# `elab.py` — an untrusted proof elaborator

## Why

`check.beta` consumes certificates in a raw, positional syntax: individual variables are
de Bruijn indices `(v N)` counting `∀`/`∃`/`gen` binders, and hypotheses are `(hyp N)`
counting `lam` binders. Writing those by hand is the dominant source of error — almost every
rejected certificate during the order-theory and arithmetic development was a miscounted
index or a binder shift tracked wrong, not a flaw in the mathematics. That cost does not
shrink as proofs get deeper; it compounds.

## The idea (and why it's safe)

The lattice's thesis is **trust by checking, not pedigree**: only `check.beta` is trusted,
so a proof-*search* engine may be arbitrarily clever because the checker re-validates its
output. The same logic applies to proof *construction*. `elab.py` is an **untrusted**
front-end: it lets proofs be written with **named binders** and compiles them to the exact
raw syntax `check.beta` consumes. A bug in the elaborator cannot make a false theorem pass —
it can only emit a certificate the trusted, minimal checker then rejects. It sits outside
the trust path entirely, exactly as `beta-rust` was throwaway scaffolding for `bc`. The
checker stays small and hand-auditable; the convenience lives elsewhere.

## The surface

S-expressions where every binder names its bound variable, and references use the name. The
elaborator maintains two scopes — individual variables (→ `(v N)`) and hypotheses
(→ `(hyp N)`) — and resolves names to de Bruijn indices automatically. A bound name shadows
the `z`/`nil` literals.

```
props : (all x P) (ex x P) (-> P Q) (& P Q) (or P Q) (= A B) bot (pred ID A) (rel ID A B)
terms : z | 0 1 2 … | (s A) (+ A B) (* A B) | nil (cons H T) (++ A B) (len A)
        (k CID A…) (f FID A…) (rec I) (y K) | NAME            ; → individual var
proofs: (gen x PF) (lam h P PF) NAME(→hyp) (use N) (have h P PF BODY)   ; local lemma
        ; (have h P pf body) names an intermediate proof pf:P as hypothesis h in body
        ; (desugars to (app (lam h:P body) pf)) — structures big trans-chains readably
        (app F A) (app* F A B …) (inst PF T) (inst* PF T1 T2 …)   ; *-forms fold the nesting
        (pair A B) (fst P) (snd P) (inl Q P) (inr Q P) (case S F G)
        (absurd Q P) (refl T) (inst PF T) (disj P) (sinj P) (unpack EPF H)
        (wit x BODY T PF)            ; BODY = the ∃-body, binding x
        (eqelim x MOT EQ BASE)       ; MOT = the Leibniz motive, binding the hole x
        (natind x MOT BASE STEP) (listind x MOT BASE STEP)
        (rec cidA cidB x MOT BASE STEP)
top   : (data CID ARITY R0 R1) (fun FID CID BODY) …  (def N P PF) …  GOAL  PROOF
        ; user data types / functions, then named lemmas, then the goal and its proof
```

### A conversion gotcha the elaborator exposes (not fixes)

The checker's conversion rule is **weak-head**: it reduces enough to expose a head
constructor, but does *not* normalize inside an argument before matching a rule. So
`m(0+Sk, …)` does **not** fire the `m(Sa,b)` rule, because the first argument is
syntactically `(p z (s k))`, not `(s _)` — even though `0+Sk` is conv-equal to `Sk`. A
transport (`eqelim`) over `(0+Sk)` therefore never reduces. The fix when a hypothesis hands
you `0+Sk = a`: coerce it to `Sk = a` first (via `trans(refl(Sk), pf)` — sound because
`refl(Sk) : Sk = 0+Sk` holds by conversion), then transport over the *syntactic* `(s k)`.
This is what unblocked `mult-positive`. With named binders the obstacle is legible; under
hand-written de Bruijn it hid behind index noise.

What the elaborator handles, that hand-writing got wrong: the `(v N)` for a variable used
under different binder depths in the same proof; the `(hyp N)` for a hypothesis used deep
inside nested `unpack`/`gen`; the implicit hole binder of an `eqelim` motive; the induction
variable of a `natind`/`rec` motive; the `∃`-body binder of a `wit`. The `inst*`/`app*` forms
fold the right-nesting of repeated instantiation and application
(`(inst* (use 2) x y z)` = `(inst (inst (inst (use 2) x) y) z)`), so applying a three-place
lemma reads as one call. The remaining manual concern is the *order* of those arguments
against a lemma's quantifiers — the elaborator does not know lemma signatures (a future
extension could) — but that is local and small next to the index bookkeeping it removes. Errors that remain are now legible: `unbound name 'foo'` and
paren-balance, instead of a silent wrong index that the checker rejects with no location.

## Use

```sh
python3 tools/elab.py < proof.proof              # print the raw certificate
python3 tools/elab.py --check ./check.exe < proof.proof   # elaborate, then run the checker
```

## Validation

`gates/elab-test.sh` elaborates every `corpus/proofs/*.proof` and asserts the
trusted `check.beta` accepts the result — so a regression in the elaborator surfaces as a
rejected certificate. The sources in `corpus/proofs/` were checked to compile **byte-identically**
to the corresponding hand-written gate certificates (`nat-add-zero`, `le-succ-mono`), and
`le-trans` — a three-lemma proof that was painful to write by hand — was authored directly
in the named-binder surface and accepted. Future gate theorems can be developed in `.proof`
and the emitted certificate pasted into `gates/test.sh`, or the source kept under
`corpus/proofs/`.
