# Packages

| Package | Responsibility |
| --- | --- |
| [package-manager/](package-manager/README.md) | Install, update, recovery, package inspection and policy review |
| [source-acquisition/](source-acquisition/README.md) | Source acquisition and custody |
| [resolver-execution/](resolver-execution/README.md) | Resolver process execution |
| [review-evidence/](review-evidence/README.md) | Compiler findings used by package review |
| [build-declarations/](build-declarations/README.md) | Shared project and dependency declarations |
| [build-output/](build-output/README.md) | Captured source/output trees and materialization |
| [compilation-inputs/](compilation-inputs/README.md) | Source composition and compilation-input custody |

These are independent sibling packages. The compiler consumes input contracts
without depending on install/update orchestration. Package operations and
review can consume compiler services. Shared process and filesystem primitives
live in [../foundation/](../foundation/README.md).
