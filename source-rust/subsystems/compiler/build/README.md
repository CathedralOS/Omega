# Project build

An ordinary project enters through [planning](planning.rs). Planning opens the captured Build source, interprets it, joins its dependency requests and opens the Product source selected by its return. [Build execution](execution/README.md) owns the activation and host bindings. The [settings decoder](build-settings/README.md) reads the returned value. [Product planning](planning/product.rs) supplies the resulting inputs to the compiler.

The [evaluated value](evaluation/values/value.rs) carries the fields, case payloads and arrays consumed by Build settings and native calling policies.

[Evaluation admission](evaluation/admission/admission.rs) checks the selected Psi declaration, its reachable operations and package permissions before the interpreter can execute a calling policy.

Build records the [resource grant and usage](evaluation/usage/usage.rs) for each evaluated program. A [native binding receipt](evaluation/receipts/native_binding.rs) ties that usage and the observed inputs to the identity of the Psi program that ran.

Build [issues filesystem grants and accounts for private staging](evaluation/filesystem/filesystem_authority.rs). Source acquisition and Build apply the same [selected-member rules](../../package-manager/source-acquisition/capture/selection/selection.rs) when constructing immutable input inventories.

Before publishing into a caller-visible directory, the host [acquires it for the input program](output_directory.rs). The lease excludes overlapping publishers. The persistent owner record prevents a different input from overwriting the same directory on a later invocation.

[Provider plans](providers/plans/provider_plan.rs) carry one provider's rows for a
service and target. Build derives the plan from checked conformances and evaluated
native bindings. Each plan and its retained bytes share the same canonical identity.
The receiver’s [service acceptance](providers/acceptance/acceptance.rs) records the
package, declaration, expected schema and per-requirement permissions. Program-entry
roles compare the declaration commitment before the target calling plan is selected.

[build.rs](build.rs) also defines a generic plan loop for the [V2 staged build design](../../../../omega-rust-v2/PLAN.md#v2s-build-model-a-chain-of-stages).
