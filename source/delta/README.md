# Delta rung

This directory owns the Delta language, its Gamma-written compiler, Delta
language cases, and adjacent source-to-Alpha-tape validation.

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

- [`tests/`](tests/) contains Delta language cases.
- [`FEATURE_LEDGER.md`](FEATURE_LEDGER.md) records Delta feature rationale and
  change control.
- `compiler/` is the owner of the future `delta_compiler.gamma`, its canonical
  Alpha tape, and refinement evidence.

The superseded Beta-written Delta-to-Gamma bridge and Darwin-native publication
tree, including the restricted Delta-written Darwin compiler prototype, are
deleted. Git history is sufficient; no compatibility owner replaces them.

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
