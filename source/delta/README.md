# Delta rung

This directory owns the Delta language, its Gamma-written compiler, and
adjacent source-to-Alpha-tape validation.

[`LANGUAGE.md`](LANGUAGE.md) is the normative Delta v1 contract. A compiler,
sample corpus, or Omega document cannot define Delta by acceptance.

## Canonical edges

```text
Gamma-written Delta compiler
  └─ gamma_compiler.tape ─▶ delta_compiler_bytecode.tape

Delta-written Omega compiler D
  └─ delta_compiler.tape ─▶ omega0_compiler_bytecode.tape
```

The first artifact accepts Delta. The second accepts Omega. They are different
compilers and must not both be called “the Delta compiler.”

## Contents and migration

- [`FEATURE_LEDGER.md`](FEATURE_LEDGER.md) records Delta feature rationale and
  change control.
- `compiler/` is the owner of the future `delta_compiler.gamma`, its canonical
  Alpha tape, and refinement evidence.

The superseded Beta-written Delta-to-Gamma bridge and Darwin-native publication
tree, including the restricted Delta-written Darwin compiler prototype, are
deleted. Git history is sufficient; no compatibility owner replaces them.
The associated 43-file corpus was also deleted: it had no runner and mixed
native-backend slices, retired proof scripts, demonstrations, and unresolved
language proposals. A compact positive/negative suite will be derived from the
Q13-frozen contract and owned by the real compiler edge instead of restoring
that corpus.

## Boundaries

- Delta is independent of Omega even when spelling overlaps.
- The Delta compiler is written in Gamma and emits exact Alpha tape directly.
- Delta-written `D` implements full Omega and may generate a slow,
  conservatively lowered `omega₀` tape.
- All fixed capacities are source-visible bounds, explicit profile parameters,
  or private budgets whose exhaustion is `Incomplete` and publishes no tape.
- Shell and Python may invoke tests or stamp tapes. They may not parse, lower,
  manufacture semantic evidence, or become compiler stages.
- The Rust compiler remains a comparator, not a producer in the canonical
  sequence.

Active work is tracked in
[`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `compiler/` | The sole owner of the future Gamma-written compiler accepting Delta and its exact Alpha-tape edge. | Replace only atomically with the admitted immediate-predecessor compiler edge. |

The root retains only the normative language contract, its feature/change
ledger, and this owner map. Proposed programs without a runner are not retained
as tests.
