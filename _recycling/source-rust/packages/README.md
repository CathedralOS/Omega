# Packages

| Package | Responsibility |
| --- | --- |
| [package-manager/](package-manager/README.md) | Install, update, recovery, package inspection and policy review |
| [source-acquisition/](source-acquisition/README.md) | Source acquisition and custody |
| [resolver-execution/](resolver-execution/README.md) | Resolver process execution |
| [review-evidence/](review-evidence/README.md) | Compiler findings used by package review |
| [build-declarations/](build-declarations/README.md) | Shared project and dependency declarations |
| [build-output/](build-output/README.md) | Captured source/output trees and materialization |
| [package-compilation/](package-compilation/README.md) | Source composition and compilation-input custody |

These packages are independent siblings. The compiler reads input contracts
without depending on install/update logic; package operations and review
may call compiler services. Shared process and filesystem helpers live in
[../foundation/](../foundation/README.md).
