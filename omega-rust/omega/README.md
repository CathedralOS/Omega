# Omega domains

Each directory owns one kind of work:

| Directory | Responsibility |
| --- | --- |
| [application/](application/README.md) | The `omega` router: parse a command, hand it to its owner, report the outcome |
| [build-system/](build-system/README.md) | Runs a project: prepares it, executes its `build.omg`, calls the compilers and publishes their outputs |
| [compilation/](compilation/README.md) | The compilers and their machinery: pipeline, representations, realization and backend |
| `interpreter/` | Hosted interpretation: the Terminal runtime that runs programs and the native-provider runtime it hosts |
| [packages/](packages/README.md) | Package operations, acquisition, review and compilation inputs |
| [host-support/](host-support/README.md) | Bounded processes, filesystem custody and publication mechanisms |

The application is Cargo package `omega` in `application/`; run it from the
workspace root with `mbx run -p omega -- run <root.omg>`.

These directories group independent packages by responsibility. Each package
keeps its own manifest and API; Cargo dependencies enforce the boundaries.
The build system in `build-system/` calls the compilers; compiler machinery,
including its independent IR packages, is in `compilation/`. Hosted program
execution is in `interpreter/`, a library any embedder can use without the
`omega` executable. Generic primitives are in [Psi foundation](../psi/foundation/README.md).
Native identity and optimization records are in `compilation/representations/`;
product measurements and request records are in `compilation/products/`.

Source processing through Terminal Psi is in [Psi](../psi/README.md),
including [target-neutral interpretation](../psi/interpreter/README.md).
The [pipeline contract](../pipeline.md) defines the Psi/Omega firewall.
The bundled libraries are in [`../../source/library/`](../../source/library/);
the Rust tree holds compiler and runtime machinery, not copies of them.
