# Computed descent recursion

Status: design, 2026-11-27. Implementation is staged below; the
`TASKS.md` bullet under **PROOF-KERNEL-CORE** about the natural order's
well-foundedness tracks it.

## What the spec asks

[Computed recursive arguments in proofs](../../spec/language/termination.md#computed-recursive-arguments-in-proofs):
a proof-only machine with `terminates by` that calls itself at a computed
natural owes, at each call, the strict descent `next < a` in the selected
order (#6267 checks it). Elaboration must preserve the recursive equations
for all admitted inputs. It may use the order's accessibility proof, or
introduce a private step counter. With a counter, it must still check the
authored descent and prove that the counter suffices on every reachable
call. The equations and laws of the source machine then hold independently
of which sufficient bound was used.

This design takes the step-counter path. The kernel has no accessibility
predicate, and adding an indexed inductive with proof-relevant arguments to
every definition would put proofs inside definitions, which stage 05 builds
before stage 06 proves anything.

## Today

Stage 05's `by_fuel` defines a machine `f(x̄)` with ranked formal `x_r` as
`F(x_r, x̄)`, where `F` recurses on the fuel `k`:

```
F(0,   x̄) = B[call(ē) := d]            d: a fixed value of the result
F(S k, x̄) = B[call(ē) := F(k, ē)]
```

`B` is the body. A law about `f` is proved for every `k` by induction on it.
That is sound, but nothing proves `F(k, ē) = f(ē)` at a call, so the source
equation `f(x̄) = B[call(ē) := f(ē)]` is not available to a law. Proving it
for this `F` needs the body: at fuel zero the calls hold `d`, and only the
descent facts inside each branch of `B` show those calls unreachable.

## The elaboration

Give each unit of fuel a guard, so that extra fuel is spent by doing
nothing:

```
G(0,   x̄) = B[call(ē) := d]
G(S k, x̄) = case x_r ∸ k of
              0   -> G(k, x̄)                 x_r <= k: more fuel than needed
              S _ -> B[call(ē) := G(k, ē)]
f(x̄)      = G(x_r, x̄)
```

`∸` is the recognized order's truncated subtraction (`natural_order`, core's
`saturating_sub`). The guard is a definition by cases of its own, over the
guard value and every variable of the fuel's case, like any dispatch on a
computed value: stage 06's normal form computes only such definitions, so
the guard computes once its value is a case and stays folded otherwise.
Stage 06 proves a law on fuel by splitting each successor case on the guard
value: at `0` the law at one unit less and the same arguments is the goal,
and otherwise the body's script runs unchanged.

### Bound: the counter suffices

```
Bound_f(k, x̄):  x_r ∸ k = 0  ->  G(k, x̄) = G(x_r, x̄)
```

It is proved by induction on `k`, and its proof never looks at `B`:

- `k = 0`: `x_r ∸ 0` computes to `x_r`, so `x_r = 0` and both sides are
  `G(0, x̄)`.
- `k = S k'`, split on `x_r ∸ k'` keeping the equation:
  - `0`: the guard takes `G(k', x̄)`, and the hypothesis at `k'` (whose
    premise is this branch's equation) closes it.
  - `S w`: then `k' < x_r` (core's `lt_of_sub_pos(k', x_r, w)` gives
    `S k' ∸ x_r = 0`), and the premise says `x_r ∸ S k' = 0`, so
    antisymmetry gives `x_r = S k'`. Rewriting the fuel position of the
    right side by it unfolds `G(S k', x̄)`, whose guard the split's
    equation `x_r ∸ k' = S w` computes to the body: the left side.

(`add` recurses on its first argument, so `x_r = add(k', S 0)` from
`sub_eq_add` would not compute to `S k'`; antisymmetry avoids `add`.)

Core states the two steps together as `succ_of_sub_between(a, k, w)`:
`a ∸ k = S w` and `a ∸ S k = 0` give `a = S k`, proved from
`lt_of_sub_pos` and the new `le_antisymm`.

`Bound` is a law stage 05 states for each machine on fuel, with this
script (`LawProof::Cases` on the bound, a `LawSplit` on the guard value, a
`Hypothesis` citation and a `Statement` citation of `succ_of_sub_between`).
Stage 06 proves it like any other law. The lemma is recognized the way
`saturating_sub` is, by its statement over the recognized order exactly; a
program whose natural data lacks it is refused as not supported, naming the
missing lemma. This is the "proved once" part: the order's lemma is proved
once in core (or in the program's copy), and each machine's `Bound` reuses
it.

No call names the lemma, so it would not be read on its own. It is the core
vocabulary row `NaturalFuelBound`: stage 02 demands its body whenever the
naturals are demanded, and stage 05 reads it from the copy of core's
naturals the program reads from.

With `Bound` and the descent each call owes, the counter suffices on every
reachable call: a call at `ē` inside `G(S k', x̄)` with `x_r ∸ k' ≠ 0` and
`ē_r < x_r` has `ē_r ∸ k' = 0` (`le_of_succ_le`-style reasoning, from
`S ē_r ∸ x_r = 0` and `x_r = S k'`), so `G(k', ē) = f(ē)`.

### Equation: what laws use

```
Equation_f(x̄):  premises(x̄)  ->  f(x̄) = B[call(ē) := f(ē)]
```

Stage 05 states it per machine with a script that follows `B`'s own
dispatches: the cases of `x_r` (at `0` every call is unreachable, because
its descent `S ē_r ∸ 0 = 0` is a constructor clash), and at each call
site a citation of that call's descent statement and of `Bound_f(pred x_r,
ē)`. A guard at `S r'` needs `S r' ∸ r' = S 0`, which core's
`sub_succ_left(r', r')` gives with `sub_self(r')` for its premise; it is
recognized like `sub_eq_add`.

### Laws

- A law about `f` that holds for every bound keeps its proof: induction on
  the bound, now with the guard split in each successor case, whose `0`
  branch is the hypothesis at the same arguments. Stage 06's `Split::Fuel`
  gets that split; the script is unchanged.
- A law that needs the equations, such as reconstruction for a
  repeated-subtraction `div`, is proved about `f` itself: stage 06 may
  rewrite `f(x̄)` by `Equation_f` under the law's premises, and a law that
  cites itself at a computed argument owes the same descent and is proved
  by induction on a bound with premise `x_r ∸ k = 0` (`LawProof` gains that
  form).

## Steps

1. The guard, with the `Split::Fuel` change so every existing law on fuel
   stays judged (landed first, on its own), then `Bound`. Fixtures: the
   repeated-subtraction `mod` compiles with its `Bound` judged; a copy of
   `Nat` without `succ_of_sub_between` is refused for the missing lemma.
2. `Equation_f`, and its use by the prover. Fixture: a law about the
   repeated-subtraction `mod` that holds only with enough fuel, such as
   `mod(a, b) = mod(a ∸ b, b)` for `b <= a`, compiles; its twin without the
   premise is refused.
3. Laws that recurse at computed arguments. Fixture: reconstruction of a
   repeated-subtraction `div` and `mod`; its twin citing itself at an
   argument that does not descend is refused at that citation.

The `TASKS.md` bullet closes with step 3.
