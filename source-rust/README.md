# Rust Omega compiler

This tree contains Omega's Rust development and comparison compiler. Delta implements the Epsilon evaluator. Epsilon is used to write the first Omega compiler D, which builds the Omega-written product. The Omega sources live in [source/psi](../source/psi) and [source/omega](../source/omega). The [bootstrap source map](../bootstrap/README.md) describes how they fit into the bootstrap.

A release or bootstrap input set can omit the Rust compiler when its dependency closure contains no artifact, build step or path supplied by this checkout. That includes finding the [bundled library](../source/library) independently. Each compiler's outputs earn trust through checks of meaning, refinement, artifact bytes, manifest identities and proof sidecars.

## Follow a compilation

| Start here | Responsibility |
| --- | --- |
| [Application](application/src/main.rs) | Parse an `omega` invocation and dispatch the command. |
| [Project build](subsystems/compiler/build/README.md) | Interpret a Build plan, supply its inputs and request the next compilation or final artifact. |
| [Compiler](subsystems/compiler/compiler.rs) | Select the requested product. Source can produce portable Psi. Source or an authorized saved Psi artifact can request runtime admission or a native image. |
| [Source to Psi](subsystems/compiler/source-to-psi/README.md) | Run the numbered source stages from lexing to checked Psi publication. |
| [Portable Psi](subsystems/compiler/psi/codec/README.md) | Encode a published program as a portable `.psi` file and reopen it for a receiving host. |
| [Psi to native](subsystems/compiler/psi-to-native/README.md) | Admit the program and its providers, then run the numbered native stages for the receiving target. |
| [Interpreter](subsystems/interpreter/README.md) | Execute admitted Psi using the services supplied by its host. |
| [Package manager](subsystems/package-manager/README.md) | Acquire package sources and maintain dependency, review and publication records. |

The normal source-to-native route passes checked Psi in memory. Saving `.psi` and
reopening it are explicit file operations. A requested native stage validator
checks the original inputs retained for that emission before releasing the image.
[Compiler requests](subsystems/compiler/README.md) · [Object verification](subsystems/compiler/psi-to-native/verification/published-objects/)

## Where code belongs

Each compiler keeps its driver files at its root and its numbered transformations in `pipeline/`. Put the data passed between its stages in the corresponding [source representations](subsystems/compiler/source-to-psi/representations) or [native representations](subsystems/compiler/psi-to-native/representations). Shared consumers can use the representation or a verifier for its evidence directly. For example, Build can request [admitted scalar values](subsystems/compiler/psi-to-native/pipeline/00_psi-to-admitted-native-inputs/scalar-applications) without taking on image production.

[Compiler foundations](subsystems/compiler/foundation) hold compiler identities and vocabulary used across these boundaries. The [Rust foundation](foundation) provides cross-subsystem facilities, including arenas, exact arithmetic, record storage, byte encoding and [host filesystem operations](foundation/filesystem/src/lib.rs).

`_recycling/` directories retain material for reference or rewrite. A returning implementation gets an active owner and is compiled there. The [admission check](../tools/ci/check_recycling.py) rejects active Cargo or Rust paths into the drawer.

Cargo's CLI package is `omega` in [application](application/Cargo.toml). Compiler integration tests live in [subsystems/compiler/tests](subsystems/compiler/tests/README.md), with shared programs under [tests/fixtures](../tests/fixtures). Repository commands and validation policy are in [AGENTS.md](../AGENTS.md).
