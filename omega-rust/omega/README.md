# Omega domains

Choose the domain that owns the use case:

| Directory | Responsibility |
| --- | --- |
| [application/](application/README.md) | The `omega` router: parse a command, hand it to its owner, report the outcome |
| [build-system/](build-system/README.md) | Runs a project: prepares it, executes its `build.omg`, calls the compilers and publishes their outputs |
| [compilation/](compilation/README.md) | The compilers and their machinery: pipeline, representations, realization and backend |
| `interpreter/` | Hosted interpretation: the Terminal runtime that runs programs and the native-provider runtime it sponsors |
| [packages/](packages/README.md) | Package operations, acquisition, review and compilation inputs |
| [foundation/](foundation/README.md) | Primitives shared across these domains |

The application is Cargo package `omega` in `application/cli/`; run it from the
workspace root with `mbx run -p omega -- run <root.omg>`.

These directories group independent packages by responsibility. Each package
retains its manifest and API; Cargo dependencies enforce the boundaries.
The build system lives within `build-system/` and calls the compilers; compiler
machinery lives within `compilation/`, including its independent IR packages. Hosted program execution lives within `interpreter/`, a library any
embedder can use without the `omega` executable. Shared utilities remain in `foundation/`.

Source processing through Terminal Psi remains in [Psi](../psi/README.md),
including [target-neutral interpretation](../psi/interpreter/README.md).
The [pipeline contract](../pipeline.md) defines the Psi/Omega firewall.
Canonical bundled libraries live in [`../../source/library/`](../../source/library/);
the Rust tree contains compiler/runtime machinery, not duplicate library packages.
