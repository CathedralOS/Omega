# Math Proofs

A theorem ladder written as Omega proof machines: empty-body machines whose
`requires`/`ensures` contracts state mathematical facts, in the style of
Chapter 10 (compile-time proofs). Each machine names its Lean analog in a
comment.

Check it (no output is produced; proof machines emit no runtime code):

```
omega --check samples/math_proofs/main.omg
```

The ladder runs from constant arithmetic (L0) up through proof views (L6).
Every theorem here is TRUE. The matching FALSE twins live in
`canaries/pending/proofs/` (and `canaries/fail/proofs/` for the rungs the
contract refutation pass already rejects); they are the acceptance tests for
the entailment engine tracked in `wiki/proof_engine_roadmap.md`.
