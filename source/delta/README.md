# Delta rung

This directory owns the Delta language corpus, the Delta-produced compiler,
its lower-rung meaning, and its publication evidence.

Delta is the final compiler stage before Omega:

```text
Gamma builds/publishes the Delta-produced compiler
Delta-produced compiler + C → omega₀
omega₀ + the same C → omega
```

`C` is the production compiler source closure, deliberately written with a
compositional subset of ordinary Omega. There is no separately named bridge
compiler between Delta and `omega₀`.

## Contents

- [`compiler/`](compiler/) contains the canonical Delta-written compiler source.
- [`tests/`](tests/) contains the executable Delta language corpus.
- [`meaning/`](meaning/) contains the lower-rung Delta-to-Gamma elaboration and
  its byte transport helpers.
- [`compiler/validation/`](compiler/validation/) contains the compiler's exact
  source-closure records, lower-rung publication verifier and driver, artifact
  custody checks, and their focused tests.
- [`FEATURE_LEDGER.md`](FEATURE_LEDGER.md) tracks Delta-language facilities
  justified by the compiler stage and the ordinary-Omega surface used by `C`.

[`compiler/main.alp`](compiler/main.alp) is the current canonical Delta compiler
source. Its fixed storage and host I/O choices are
implementation/resource commitments only where the Delta contract explicitly
retains them.

The retired `build/delta{0,1,2}.exe` files had no live consumer and were not
members of the canonical source closure. Git history retains them. A published
compiler artifact belongs under `compiler/artifacts/` only after its exact
producer edge and custody receipt exist; publication machinery belongs beside
it under `compiler/validation/`.

## Boundaries

- Omega-like spelling does not make Delta an Omega subset.
- The Delta-produced compiler may lower conservatively, but accepted source
  retains exact ordinary Omega meaning.
- Unsupported input and resource exhaustion reject before publication.
- An ambient assembler/linker result may receive an exact custody receipt, but
  it gains no compiler authority without separately checked direct refinement.
- Shell and Python files may drive tests or verify recorded receipts. They are
  not semantic compiler stages and must be replaceable by direct invocation of
  the compiler artifacts they coordinate.
- The Rust compiler at [`../omega-rust/`](../omega-rust/) is a comparator, not a
  producer in the bootstrap chain.

Active work is tracked in [`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
