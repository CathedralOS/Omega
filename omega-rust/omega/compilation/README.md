# Compilation

The compilers and their machinery, grouped by responsibility. The build system
that calls them lives in [../build-system/](../build-system/README.md).

| Directory | Responsibility |
| --- | --- |
| [compiler/](compiler/README.md) | Product compilation coordination; Cargo package `compiler` |
| `native-compiler/` | The native compiler: Terminal Psi to a native image; Cargo package `native-compiler` |
| [products/](products/README.md) | Requests, retained results, reports and publication |
| [pipeline/](../../pipeline.md#connected-program-route) | Numbered transformations and optimization passes |
| [representations/](representations/README.md) | Independent native program forms and their support packages |
| `realization/` | Provider planning, selected execution and trust reconstruction |
| `backend/` | Target, ISA, ABI, encoding, layout, image and runtime mechanisms |

The compiler crate sequences independent owners. Representations retain their own
manifests and APIs, separate from the transformations that consume them.
Package-owned source composition lives in
[../packages/package-compilation/](../packages/package-compilation/README.md).

[Psi](../../psi/README.md) owns target-neutral source processing through Terminal
Psi. The [pipeline contract](../../pipeline.md) documents that boundary and the
remaining work to make realization consume Terminal Psi exclusively.
