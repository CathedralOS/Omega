# Math Proofs

A theorem ladder written as Omega proof machines: empty-body machines whose
`requires`/`ensures` contracts state mathematical facts, in the style of
Chapter 10 (compile-time proofs). Each machine names its Lean analog in a
comment.

Check it (no output is produced; proof machines emit no runtime code):

```
omega --check bootstrap/gates/corpus/math_proofs/main.omg
```

The ladder runs from constant arithmetic through ranked induction. Every
theorem here is true. Matching false twins under `tests/canaries/fail/proofs/` are
acceptance tests for the entailment engine described in
`wiki/proof_engine_roadmap.md`.
