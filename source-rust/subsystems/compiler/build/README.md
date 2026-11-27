# Project Build

The package manager starts a project Build with `project_build::prepare` and passes the captured source to [the project-build crate root](project_build.rs). `project_build::interpret` resumes the authored Build when it requests a dependency or provider. Its returned stage opens the selected Product with `open_product`. [Product planning](planning/product.rs) passes the selected source, bindings and settings to the source and native compilers.

An ordinary [release program](../../../../wiki/spec/build/declarations.md#releases-with-independent-products) coordinates independent products.
The [child filesystem contract](../../../../wiki/spec/build/scoped_execution.md#supplying-a-filesystem-to-a-child-build) specifies how a parent supplies an owned or borrowed view. The [implementation task](../../../../TASKS.md#immediate-product-closure) tracks the handoff. [Live network grants](../../../../wiki/spec/build/scoped_execution.md#live-networking-in-a-build) follow the Build invocation. [Program launches](../../../../wiki/spec/build/program_launch.md) require separately bound executable grants.

| Directory | Responsibility |
| --- | --- |
| [inputs](inputs/README.md) | Assemble captured package and toolchain sources, resolve imports and record the bytes used. [Build declarations](inputs/declarations/README.md) describe inputs and requests. [Captured trees](filesystem/trees/README.md) hold immutable inputs and staged outputs. |
| [evaluation](evaluation/README.md) | Admit and interpret the authored Build with its services. [Authority](evaluation/authority) limits filesystem access and service use. [Accounting](model/accounting/README.md) records grants and charges. |
| [settings](evaluation/settings/README.md) | Decode the returned value into target settings, [optimization requests](model/optimization_requests.rs) and [behavior exclusions](model/behavior_exclusions.rs). |
| [providers](providers) | Record the receiver’s [acceptance](providers/acceptance.rs), the provider’s [plan](providers/plans.rs) and the [selected plans](providers/selections). |
| [publication](publication/publication.rs) | Publish the selected product to an [owned output directory](publication/output_directory.rs) and retain the native output with its canonical Psi artifact. |

Build compiles as ten crates. Each split follows a rule that `mbx xtask check-boundary` or the dependency graph enforces:

| Crate | Directory | Why it is separate |
| --- | --- | --- |
| `build-model` | [model](model/model.rs) | Evaluation contracts, accounting, receipts, values, admission, behavior exclusions and optimization requests. The source compiler, the native compiler and the interpreter depend on it, so it names no native target and runs no Build. |
| `build-providers` | [providers](providers/providers.rs) | Provider acceptance, plans and selections. Plans name native targets, which the source compiler must not reach. |
| `build-time-evaluation` | [build-time](build-time/build_time.rs) | Scalar evaluation, invocations and value projection. These depend on the interpreter, which depends on `build-model`. |
| `build-filesystem` | [filesystem](filesystem/filesystem.rs) | Filesystem grants, captured trees and consent requests. Package review reads them without depending on the compilers. |
| `build-program-launch` | [evaluation/authority/program](evaluation/authority/program/src/lib.rs) | The only Build code that uses `unsafe`, for Linux process confinement. |
| `package-compilation` | [inputs](inputs/inputs.rs) | Package inputs, Build declarations and normalized settings. `compiler` depends on it and `project-build` depends on `compiler`. |
| `build-evaluation-result` | [evaluation/result](evaluation/result/result.rs) | The one Build crate allowed to read source stage 07. |
| `build-execution` | [evaluation](evaluation/execution.rs) | Runs the authored Build and decodes its settings. `project-build` unit tests drive the package manager, which depends on this crate, so merging it into `project-build` would give those tests two copies of each execution type. |
| `product-publication` | [publication](publication/publication.rs) | The scoped reader of the native artifact. The package manager must not depend on it. |
| `project-build` | [project_build.rs](project_build.rs) | Planning and the package manager's entry points. |
