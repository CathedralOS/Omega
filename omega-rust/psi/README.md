# Psi domains

Psi implements target-neutral Omega meaning: compilation from source to
Terminal Psi and interpretation of checked or Terminal programs. It does not
own an application, package acquisition, native realization, or a second product
compiler driver.

| Directory | Responsibility |
| --- | --- |
| [compilation/](compilation/README.md) | Source transformations, semantic checking, proof, compile-time elaboration and Terminal publication |
| [interpreter/](interpreter/README.md) | Checked-program and verified-Terminal execution engines |
| [foundation/](foundation/) | Shared arenas, symbols, source, numerics and semantic vocabulary |

These domains group independent packages; they do not merge their APIs.
Compilation owns the durable [program representations](compilation/representations/README.md),
which interpreters and Omega consumers can depend on without depending on a
compiler coordinator. Shared judgments are similarly reusable libraries.
Compile-time evaluation consumes the checked interpreter; the interpreter does
not call the compilation workflow back.

The [connected pipeline](../pipeline.md) names each transformation and its
entrance. Omega's product driver sequences compilation; Psi stages own the
language decisions. [Terminal Psi](../../wiki/spec/terminal-psi/product.md)
is the portable boundary. Omega consumes verified Terminal Psi, not a
checked-tree native fallback. Psi packages must not depend on Omega backends,
ABI, instruction, physical-storage or installation representations.

Reference interpretation is not a fallback for unsupported native lowering.
Cross-layer interpreter/native comparisons belong in test harnesses, and
successful interpretation alone establishes neither native coverage nor
fixed-work evidence.
