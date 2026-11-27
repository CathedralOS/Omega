# Project Build

The package manager starts a project Build with `project_build::prepare` and passes the captured source to [the project-build crate root](project_build.rs). `project_build::interpret` resumes the authored Build when it requests a dependency or provider. Its returned stage opens the selected Product with `open_product`. [Product planning](planning/product.rs) passes the selected source, bindings and settings to the source and native compilers.

An ordinary [release program](../../../../wiki/spec/build/declarations.md#releases-with-independent-products) coordinates independent products.
The [child filesystem contract](../../../../wiki/spec/build/scoped_execution.md#supplying-a-filesystem-to-a-child-build) specifies how a parent supplies an owned or borrowed view. The [implementation task](../../../../TASKS.md#immediate-product-closure) tracks the handoff. [Live network grants](../../../../wiki/spec/build/scoped_execution.md#live-networking-in-a-build) follow the Build invocation. [Program launches](../../../../wiki/spec/build/program_launch.md) require separately bound executable grants.

| Directory | Responsibility |
| --- | --- |
| [inputs](inputs/README.md) | Assemble captured package and toolchain sources, resolve imports and record the bytes used. [Build declarations](inputs/declarations/README.md) describe inputs and requests. [Captured trees](inputs/trees/README.md) hold immutable inputs and staged outputs. |
| [evaluation](evaluation/README.md) | Admit and interpret the authored Build with its services. [Authority](evaluation/authority) limits filesystem access and service use. [Accounting](model/accounting/README.md) records grants and charges. |
| [settings](settings/README.md) | Decode the returned value into target settings, [optimization requests](settings/optimizations/requests.rs) and [behavior exclusions](model/behavior_exclusions.rs). |
| [providers](providers) | Record the receiver’s [acceptance](providers/acceptance.rs), the provider’s [plan](providers/plans.rs) and the [selected plans](providers/selections). |
| [publication](publication/publication.rs) | Publish the selected product to an [owned output directory](publication/output_directory.rs) and retain the native output with its canonical Psi artifact. |
