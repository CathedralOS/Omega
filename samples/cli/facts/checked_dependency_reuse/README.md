# Checked dependency reuse

Owner: `CHECKED-PACKAGE-REUSE`.

`dependency` publishes `seven`, which returns 7. The application calls that
machine and accepts only that result. The matching arm exits. The fallback
traps. `Expected exit: 0`.

Immutable declaration pages and their fact-context index are shared across
independent flow groups. Each group keeps its own local facts, fixed-point
join, and unknown-input fallback. A second consumer in the same compiler
session reuses the dependency body's entailment judgment instead of judging
that body from source again. Whole-program reuse still requires the typed
input and the complete preparation admission to match, so a different caller
does not take the first program's judgment.

```sh
omega run --both samples/cli/facts/checked_dependency_reuse/main.omg
```
