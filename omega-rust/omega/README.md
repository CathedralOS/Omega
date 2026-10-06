# Omega domains

Choose the domain that owns the use case:

| Directory | Responsibility |
| --- | --- |
| [application/](application/README.md) | CLI entry, command workflows and application policy files |
| [compilation/](compilation/README.md) | Compiler coordination, pipeline, representations, realization and backend |
| [packages/](packages/README.md) | Package operations, acquisition, review and compilation inputs |
| [foundation/](foundation/README.md) | Primitives shared across these domains |

The application is Cargo package `omega` in `application/cli/`; run it from the
workspace root with `mbx run -p omega -- run <root.omg>`.

These directories group independent packages by responsibility. Each package
retains its manifest and API; Cargo dependencies enforce the boundaries.
Compiler machinery lives within `compilation/`, including its independent IR
packages. Shared utilities remain in `foundation/`.

Source processing through Terminal Psi remains in [Psi](../psi/README.md),
including [target-neutral interpretation](../psi/interpreter/README.md).
The [pipeline contract](../pipeline.md) defines the Psi/Omega firewall.
Canonical bundled libraries live in [`../../source/library/`](../../source/library/);
the Rust tree contains compiler/runtime machinery, not duplicate library packages.
