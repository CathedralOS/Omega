# Delta rung

This directory owns the Delta language corpus, the Delta-produced compiler,
its lower-rung meaning, and its publication evidence.

Delta is the final compiler stage before Omega:

```text
Gamma builds/publishes the Delta-produced compiler
Delta-produced compiler + C → omega₀
omega₀ + the same C → omega
```

`C` is the production compiler source closure, written in ordinary Omega under
the `Ωself` authoring profile. There is no separately named bridge compiler
between Delta and `omega₀`.

## Contents

- [`samples/`](samples/) contains the executable Delta corpus and current
  canonical compiler source experiment.
- [`meaning/`](meaning/) contains the lower-rung Delta-to-Gamma elaboration and
  its byte transport helpers.
- [`build/`](build/) contains provisional artifacts. They are inputs to
  reconstruction, never authorities.
- [`source-closures/`](source-closures/) contains the exact canonical compiler
  source and tool manifests.
- [`FEATURE_LEDGER.md`](FEATURE_LEDGER.md) tracks Delta-language facilities
  justified by the compiler stage and direct `Ωself` input requirement.

[`samples/lowermachine.alp`](samples/lowermachine.alp) is the current canonical
Delta compiler source experiment. Its fixed storage and host I/O choices are
implementation/resource commitments only where the Delta contract explicitly
retains them.

## Boundaries

- Omega-like spelling does not make Delta an Omega subset.
- The Delta-produced compiler may lower conservatively, but accepted `Ωself`
  source retains exact ordinary Omega meaning.
- Unsupported input and resource exhaustion reject before publication.
- Shell and Python files may drive tests or verify recorded receipts. They are
  not semantic compiler stages and must be replaceable by direct invocation of
  the compiler artifacts they coordinate.
- The Rust compiler at [`../omega-rust/`](../omega-rust/) is a comparator, not a
  producer in the bootstrap chain.

Active work is tracked in [`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
