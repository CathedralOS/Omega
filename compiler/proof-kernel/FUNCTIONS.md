# Design: user-defined recursive functions over user types (proof-kernel frontier)

Status: **IMPLEMENTED — arity 0/1/2 constructors AND 1/2 arguments, verified across all
five defense layers** — check.beta (table) + checker.gamma + checker_typed.gamma (inline
rules) + eq.beta; gate, soundness, checker diamond, type-safety, and the semantics
diamond. Universal theorems over user functions prove by induction. A number type with
USER-DEFINED arithmetic is shown to be a **commutative semiring** inside the checker:
`add` is a commutative monoid (`∀x. add(x,Z)=x`, associativity, `∀x∀y. add(x,y)=add(y,x)`),
and a `mult` *defined in terms of* `add` (functions calling functions) computes (`2·3=6`),
satisfies `∀x. mult(x,Z)=Z`, **distributes** (`mult(a+b,c)=mult(a,c)+mult(b,c)`), and
**commutes** (`∀a∀b. mult(a,b)=mult(b,a)`). The commutativity proof is the deepest: it
needs a left-expansion lemma `mult(x,Sy)=add(mult(x,y),x)` and must induct on the inner
variable with the other left free (a single outer `gen`), because the checker's `gen`
imposes an eigenvariable condition — no hypothesis in scope may carry a free individual
variable. A binary application is `(f fid x y)` = `FAPP(fid, FBUNDLE(x,y))`; the 2nd
argument is `(y k)` = `PAR` in rule bodies, threaded through recursion. Next: N-ary (3+)
arguments. The design below is the record.

This is the trust anchor's stated #1 frontier
(`README.md` / `proof-kernel/README.md`): today a `data`-declared type's constructors are inert
data with structural equality + induction (`rec`), but there is no way to *define a
function* over them (a `mirror`/`size`/`pred`) whose equations reduce. Adding it makes
*theorems* over user types provable, not just their induction principles.

This note fixes the design so the implementation is mechanical — and, per the
single-responsibility discipline, so it can be added **without bloating** the three
checkers or `eq.beta`.

## What it must do

Let a user declare a function by one rewrite rule per constructor of its (single)
argument type, e.g. doubling a Peano-shaped user `Nat` (`Z = (k 2)`, `S x = (k 3 x)`):

```
( fun 7 2 (k 2)          z )                 ; f(Z)      = z         (cidZ = 2)
( fun 7 3 (k 3 (v 0))    (s (s (rec 0))) )   ; f(S x)    = s s (f x) (cidS = 3)
```

A function is `(fun FID CID pattern body)` — one declaration per constructor. `pattern`
is `(k CID arg-vars…)` over fresh pattern variables (de Bruijn `(v 0) … (v n-1)`); `body`
is a term over those variables plus **`(rec i)`**, the recursive call `f(arg_i)`. An
application is written `(f FID t)` and is an ordinary *term* (tags extend the existing
`30…51` term space — say `52 FUN`, `53 FAPP`).

## The one new operation: `reduce_fun` (single responsibility)

Do **not** scatter reduction logic through `normalize`. Add exactly one helper:

```
reduce_fun(fid, arg) -> term | STUCK
  arg' = normalize(arg, fuel-1)
  if arg' is (k CID v0 … vk):           ; a fully-applied constructor head
      find the rule (fun fid CID pattern body)   ; else STUCK (no rule / open arg)
      return instantiate(body, [v0 … vk], fid)   ; substitute args + rewrite (rec i)
  return STUCK
```

`normalize`'s only change is one new arm: `tag == FAPP -> let r = reduce_fun(fid, arg);
if r==STUCK then alloc(FAPP, fid, normalize(arg)) else normalize(r, fuel-1)`. Everything
else in `normalize` is untouched. `instantiate(body, args, fid)` is the second (and last)
new helper: a structural walk replacing pattern var `(v i)` with `args[i]` and `(rec i)`
with `(f fid args[i])`. It reuses the existing `subst_term` shape — it is **not** new
de Bruijn machinery, just a different leaf action.

So the feature is **two small procs + one `normalize` arm**, per checker. That is the
whole footprint; the de Bruijn family (`shift_*`, `subst_*`, `free_iv*`, `wit_*`) is
unchanged.

## Totality (the load-bearing constraint)

`normalize` is already **fuel-bounded** (`normalize(t, fuel) -> normal | OutOfFuel`), and
that is the only thing keeping the checker total. `reduce_fun` must inherit it: every
recursive unfold spends fuel, so a non-terminating user definition simply runs out of
fuel and the application stays `STUCK` — never a hang, never unsoundness (a stuck term is
just not definitionally equal to anything it didn't reduce to). We do **not** need to
prove user definitions terminating; the fuel bound makes admitting them safe. (A later
refinement could *require* structural decrease on `(rec i)` — `arg_i` a strict subterm —
to guarantee progress, but it is not needed for soundness.)

## How it reaches proofs (free, via the conversion rule)

Nothing in `infer` changes. Because function applications now reduce under `normalize`,
the existing **conversion rule** (`refl` / `type_eq`'s `conv_eq`) immediately proves
ground equations like `f(S Z) = S S Z` by computation, and `eqelim` + `rec` (induction)
prove the universally-quantified laws (e.g. `∀n. f(n) = n + n`) exactly as `n+0=n` is
proved today. The function layer is purely a `normalize` extension; the proof layer
already knows what to do with it.

## Mirroring across the diamond — TWO storage models (the real subtlety)

The two checkers store declarations *differently by design*, and the feature must respect
that — this is the part to get right before writing any code:

- **`check.beta` is stateful.** It already keeps `data` shapes in fixed memory tables
  (`ARITY 5505024`, `REC0/REC1`) populated by the `(data …)` decl prefix in `main`. So
  user functions get the same treatment: a rule table keyed by `(fid, cid)`, populated by
  a `(fun …)` decl prefix; `reduce_fun` looks rules up there. New work: `reduce_fun` /
  `instantiate` procs + the `FAPP` arm in `normalize` + decl parsing + `(f …)` in
  `parse_nat`.

- **`checker.gamma` / `checker_typed.gamma` are pure-functional — NO mutable table.** They
  already solved "where do constructor shapes live?" by carrying them **inline in the
  proof term**: `(Rec (Mkspec ca aa r0a r1a) (Mkspec cb ab r0b r1b) motive cA cB)`. User
  functions must follow that precedent: the application node carries its rules,
  `(Fapp arg (Frule cidZ patZ bodyZ) (Frule cidS patS bodyS))`, and `pnorm`'s new arm
  reduces using the attached rules — no global lookup, because there is nowhere to look.
  This keeps the gamma checker a pure fold and is why the two checkers can disagree on
  *representation* while the diamond still forces them to agree on every *verdict*.

- **`proof-kernel/eq.beta`** — the same `FAPP` arm in its `normalize` (stateful, like check.beta),
  so the **semantics diamond** (`semantics-diamond.sh`) keeps cross-checking definitional `=`.

That representational split is the single most important thing to settle up front: it
means the *reduction logic* (`instantiate` over a constructor head) is shared in spirit but
the *rule sourcing* (table vs. inline) is deliberately different per checker — and neither
side should grow a second responsibility (for example, check.beta should not
start inlining rules and Gamma should not grow a table), because the
representation-sensitive cross-check would then lose diagnostic independence.

## Verification plan (before it is trusted)

1. Diamond: `f(S Z) = S S Z` and `∀n. f(n) = n+n` accept in *both* checkers.
2. Soundness battery: a divergent definition (`g(Z)=g(Z)` style) must leave applications
   STUCK and must **not** let `g(Z) = anything` be proved by `refl`; a rule whose body
   mentions a non-pattern variable must be rejected at declaration.
3. Semantics diamond: the new computational equalities agree with an `interp.beta`-level
   evaluator of the same function (the proof/meaning seam, extended).
4. `eq.beta` computational checks for the new reductions.

Only when all four are green is the layer trustworthy — same bar the rest of the anchor
meets.
