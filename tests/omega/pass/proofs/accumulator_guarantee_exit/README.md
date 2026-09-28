# Ranked accumulator

After normal package-review preparation, run from the repository root:

```sh
omega run --both tests/omega/pass/proofs/accumulator_guarantee_exit/main.omg
```

Native execution and the checked interpreter must both exit 70: `climb(4, 10)`
returns 14 under its checked accumulation postcondition. The conjunctive guard
supplies the strict-descent premise, but its second condition can exit early;
termination evidence alone does not establish an exact countdown trip count.
The neighboring `runtime_ranked_accumulator_early_exit` fixture observes that
early exit by returning the accumulator rather than the conserved sum.
