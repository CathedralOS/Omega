# Structural Proofs

A theorem ladder over the roster's **recursive proof-only data** — Peano
`Nat` and generic `Seq<T>` from `omega::language::core` — proven by the
structural entailment judge (math roster N3/N4). Where `math_proofs` is an
integer ladder, this one proves facts about constructors and recursion.

Check it (proof machines emit no runtime code, so there is no output):

```
omega --check samples/cli/proofs/structural_proofs/main.omg
```

The rungs, each with its Lean analog in a comment:

- **Equality kernel** — reflexivity, constructor injectivity
  (`Succ a == Succ b ⟹ a == b`), case disjointness (ex falso from
  `Zero == Succ a`).
- **Compute mode** — unfolding a proof machine's definition on ground
  arguments (`1 + 1 == 2`, `length(Empty) == 0`).
- **Citation** — consuming a core lemma's proven ensures
  (`add_zero_right`'s right identity, whose inductive body never finitely
  unfolds for a symbolic argument).

The library lemmas the citation rung leans on — `add_zero_right` (right
identity, by structural induction) and `add_succ_law` (the successor-shift
law, an equation between applications) — are proven **in** `core/nat.omg`,
machine-checked for every importer. Every theorem here is TRUE; the FALSE
twins live in `tests/canaries/fail/proofs/`.
