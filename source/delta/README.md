# Delta rung

This directory owns the Delta language corpus, the Delta-produced compiler,
its lower-rung meaning, and its publication evidence.

Delta is the final compiler stage before Omega:

```text
Gamma evaluates the declared Delta meaning route → Delta compiler artifact
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

[`compiler/main.delta`](compiler/main.delta) is the current physical location of the
canonical Delta compiler source. D10 retired the misleading `.alp` suffix; the
path-only migration renamed it and the Delta corpus to `.delta` without changing
source bytes or path-independent closure identity. Its fixed storage
and host I/O choices are
implementation/resource commitments only where the Delta contract explicitly
retains them.

The retired `build/delta{0,1,2}.exe` files had no live consumer and were not
members of the canonical source closure. Git history retains them. A published
compiler artifact belongs under `compiler/artifacts/` only after its exact
producer edge and custody receipt exist; publication machinery belongs beside
it under `compiler/validation/`.

## Boundaries

- Familiar Omega spelling does not make Delta an Omega subset. Delta's contract
  must define every accepted form without consulting Omega documentation, and
  Delta acceptance never proves Omega meaning.
- Delta's observation profile starts with sealed input bytes and ends with exact
  artifact/diagnostic bytes plus its declared terminal outcome. Private
  producer/checker budget exhaustion is `Incomplete`, not a Delta result.
- The Delta-produced compiler may lower conservatively, but accepted source
  retains exact ordinary Omega meaning.
- Source outside Delta rejects. Language-visible exhaustion follows the Delta
  contract; private producer/checker exhaustion is `Incomplete`. None of these
  outcomes publishes an artifact.
- An ambient assembler/linker result may receive an exact custody receipt, but
  it gains no compiler authority without separately checked direct refinement.
- Shell and Python files may drive tests or verify recorded receipts. They are
  not semantic compiler stages and must be replaceable by direct invocation of
  the compiler artifacts they coordinate.
- The Rust compiler at [`../omega-rust/`](../omega-rust/) is a comparator, not a
  producer in the direct compiler sequence.

Active work is tracked in [`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
