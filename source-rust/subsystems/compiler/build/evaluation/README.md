# Build execution

This directory runs a project’s authored Build. Package acquisition supplies its captured source and grants. The source compiler judges the selected Build invocation and publishes the program that will run. A prepared evaluation keeps that program, its source and its execution profile together. [Build source compilation](../../source-to-psi/driver/compile_build.rs) · [Evaluation entry](build_evaluation.rs)

The interpreter runs one activation against those grants. Host calls can observe captured inputs, register dependencies, query Product declarations or request a provider. Project planning services each suspension and resumes the same activation. Normal return produces the configuration, binding requests, generated source and accounting for that execution. [Activation](activation.rs) · [Host bindings](host/bindings.rs) · [Planning](../project_build.rs)

Preparing a dependency records the selected package against the registration the Build supplied. Its checked imports and issued handles remain attached to the running interpreter. A later Product can consume the prepared package in its own compilation. [Checked Build stages](../../../../../wiki/spec/build/scoped_execution.md#checked-build-stages)

Build execution reads settings from the interpreter’s returned value. A project can request several deployment targets from the same returned Build. Planning keeps its package identity and uses its executed bindings when it opens each Product. [Settings decoder](../settings/settings.rs) · [Execution result](result/result.rs) · [Product requests](../planning/product.rs)

Restrictions and filesystem observations travel with the execution that issued them. Each accepted binding is tied to its original Build receiver and call. The host records the selected target and locator for a native binding. The Product compiler later checks the selection against the package scope and target. [Native binding host calls](host/bindings/native_bindings.rs) · [Package filesystem and observation types](../inputs/build_execution.rs)
