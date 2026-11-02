# Compilation

The compiler's machinery is grouped here by responsibility:

| Directory | Responsibility |
| --- | --- |
| [compiler/](compiler/README.md) | Product compilation coordination; Cargo package `compiler` |
| [build/](build/README.md) | Build coordination: requests, dependency jobs, sessions and publication; Cargo package `omega-build` |
| [products/](products/README.md) | Requests, retained results, reports and publication |
| [build-evaluation/](build-evaluation/README.md) | Build admission and execution |
| [pipeline/](../../pipeline.md#connected-program-route) | Numbered transformations and optimization passes |
| [representations/](representations/README.md) | Independent native program forms and their support packages |
| `realization/` | Provider planning, selected execution and trust reconstruction |
| `backend/` | Target, ISA, ABI, encoding, layout, image and runtime mechanisms |

The driver sequences independent owners. Representations retain their own
manifests and APIs, separate from the transformations that consume them.
Package-owned source composition lives in
[../packages/package-compilation/](../packages/package-compilation/README.md).

[Psi](../../psi/README.md) owns target-neutral source processing through Terminal
Psi. The [pipeline contract](../../pipeline.md) documents that boundary and the
remaining work to make realization consume Terminal Psi exclusively.
