# Structural Proofs

A theorem ladder over the roster's **recursive proof-only data** — Peano
`Nat` and generic `Seq<T>` from `omega::language::core` — proven by the
structural entailment judge (math roster N3/N4). Where `math_proofs` is an
integer ladder, this one proves facts about constructors and recursion.

Check it (proof machines emit no runtime code, so there is no output):

```
omega --check samples/cli/proofs/structural_proofs/main.omg
```

`build.omg` binds the hosted targets' `ProgramEntry` slots to the inert
`Main::main`, so a target compile also produces a native artifact (compile
and code-size legs only; the empty entry's run behavior is unspecified):

```
omega --target linux_x86_64 samples/cli/proofs/structural_proofs/main.omg
```

The rungs, each with its Lean analog in a comment:

- **Equality kernel** — reflexivity, constructor injectivity
  (`Succ a == Succ b ⟹ a == b`), case disjointness (ex falso from
  `Zero == Succ a`).
- **Compute mode** — unfolding a proof machine's definition on ground
  arguments (`1 + 1 == 2`, `length(Empty) == 0`).
- **Citation** — consuming a core lemma's proven ensures
  (`length_append`, whose inductive body never finitely unfolds for a
  symbolic argument).

The cited library lemma is `length_append` (`length(append s t) ==
add(length s, length(t))`), proven **in** `core/seq.omg` and machine-checked
for every importer. nat.omg's `add_zero_right` and `add_succ_law` theorems
remain package-private, and a package selecting a product may only cite
another package's public machines, so the citation rung uses the public
`seq.omg` theorem. Every theorem here is TRUE; the FALSE twins live in
`tests/omega/fail/proofs/`.
