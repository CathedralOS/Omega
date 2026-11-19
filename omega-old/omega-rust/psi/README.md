# Psi domains

Psi implements target-neutral Omega meaning: compiling source to
Terminal Psi and interpreting Terminal programs. It does not own an
application, package acquisition, native realization, or a second product
compiler driver.

| Directory | Responsibility |
| --- | --- |
| [compilation/](compilation/README.md) | Source transformations, semantic checking, proof, compile-time elaboration and Terminal publication |
| [interpreter/](interpreter/README.md) | The verified-Terminal execution engine |
| [foundation/](foundation/) | Shared arenas, symbols, source, numerics and semantic vocabulary |

These domains group independent packages; they do not merge their APIs.
Compilation owns the durable [program representations](compilation/representations/README.md),
so interpreters and Omega can depend on them without a compiler coordinator.
Shared judgments are likewise reusable libraries.
Compile-time evaluation uses the interpreter; the interpreter never calls back
into the compilation workflow.

The [connected pipeline](../pipeline.md) names each transformation and its
entrance. Omega's product driver sequences compilation; Psi stages make the
language decisions. [Terminal Psi](../../wiki/spec/terminal-psi/product.md)
is the portable boundary, and Omega consumes verified Terminal Psi.
Psi packages must not depend on Omega backends or on ABI, instruction,
physical-storage or installation representations.

Reference interpretation is not a fallback for unsupported native lowering.
Comparisons between the interpreter and native code belong in test harnesses.
Successful interpretation alone shows neither native coverage nor fixed-work
evidence.
