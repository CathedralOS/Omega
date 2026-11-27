# Rust Omega compiler

This tree contains Omega's Rust development and comparison compiler. Delta implements the Epsilon evaluator. Epsilon is used to write the first Omega compiler D, which builds the Omega-written product. The Omega sources live in [source/](../source/README.md). The [bootstrap source map](../bootstrap/README.md) describes how they fit into the bootstrap.

A release or bootstrap input set can omit the Rust compiler when its dependency closure contains no artifact, build step or path supplied by this checkout. That includes finding the [bundled library](../source/library) independently. Each compiler's outputs earn trust through checks of meaning, refinement, artifact bytes, manifest identities and proof sidecars.

## Follow a compilation

| Start here | Responsibility |
| --- | --- |
| [Application](application/src/main.rs) | Parse an `omega` invocation and dispatch the command. |
| [Source operations](subsystems/source/lib.rs) | Check the syntax of an authored file and locate source errors. |
| [Project build](subsystems/compiler/build/README.md) | Interpret a Build plan, supply its inputs and request the next compilation or final artifact. |
| [Compiler](subsystems/compiler/compiler.rs) | Select the requested product. Source can produce portable Psi. Source or an authorized saved Psi artifact can request runtime admission or a native image. |
| [Source to Psi](subsystems/compiler/source-to-psi/README.md) | Run the numbered source stages from lexing to checked Psi publication. |
| [Psi program](subsystems/compiler/psi/README.md) | Define the machine bodies, declarations and evidence shared by source publication, interpretation and native lowering. |
| [Psi artifacts](subsystems/compiler/psi/artifact/README.md) | Encode a published program as a portable `.psi` file and reopen it for a receiving host. |
| [Psi to native](subsystems/compiler/psi-to-native/README.md) | Admit the program and its providers, then run the numbered native stages for the receiving target. |
| [Interpreter](subsystems/interpreter/README.md) | Execute admitted Psi using the services supplied by its host. |
| [Package manager](subsystems/package-manager/README.md) | Prepare projects and their dependencies for Build, and maintain package review and publication records. |

The normal source-to-native route passes checked Psi in memory. Saving `.psi` and
reopening it are explicit file operations. A requested native stage validator
checks the original inputs retained for that emission before releasing the image.
[Compiler requests](subsystems/compiler/README.md) · [Object verification](subsystems/compiler/psi-to-native/verification/published-objects/)

## Where code belongs

The [source driver](subsystems/compiler/source-to-psi/driver.rs) and [native driver](subsystems/compiler/psi-to-native/driver/mod.rs) coordinate their numbered transformations in `pipeline/`. Put the data passed between its stages in the corresponding [source representations](subsystems/compiler/source-to-psi/representations) or [native representations](subsystems/compiler/psi-to-native/representations).

The [Build scalar evaluator](subsystems/compiler/build/build-time/scalars/) runs the calculations requested by a published Psi program. [Psi's execution inputs](subsystems/compiler/psi/execution-inputs/) check the supplied scalar results against the program, its providers and its target layout. The interpreter and native compiler both use those checks.

[Compiler foundations](subsystems/compiler/foundation) hold compiler identities and vocabulary used across these boundaries. The [Rust foundation](foundation) provides cross-subsystem facilities, including arenas, exact arithmetic, record storage, byte encoding and [host filesystem operations](foundation/filesystem/src/lib.rs).

The [repository archive](../_recycling/) retains material for reference or rewrite. A returning implementation gets an active owner and is compiled there. The [admission check](../tools/ci/check_recycling.py) rejects active Cargo or Rust paths into the drawer.

Cargo's CLI package is `omega` in [application](application/Cargo.toml). Compiler integration tests live in [subsystems/compiler/tests](subsystems/compiler/tests/README.md), with shared programs under [tests/fixtures](../tests/fixtures). Repository commands and validation policy are in [AGENTS.md](../AGENTS.md).
