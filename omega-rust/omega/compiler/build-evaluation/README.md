# Build evaluation

`admitted_build_program.rs` admits one selected companion build machine;
`execution` consumes that activation against its selected filesystem.
[Compilation products](../../realization/compilation-products/src/evaluated_build.rs)
owns the retained `BuildConfig` and `ComputedBuildConfig` data contracts, re-exported
here for callers. Evaluation produces those records; consumers need not import
the evaluator merely to retain its result.

The [Build-value protocol](../../package-manager/compilation-inputs/src/build_execution/build_value.rs)
owns exact vocabulary admission, initial Build facets and normalized settings,
including optimization counters and application/PCC/privileged-service fields.
One admission object follows initialization through decoding. The compiler
retains the settings whole beside separately checked root/provider/trust rows;
it does not reproduce the value protocol or reconstruct its fields.

Filesystem preparation, captured-input lifetime, observation identities and
sealed-output settlement belong to
[package compilation custody](../../package-manager/compilation-inputs/src/build_execution.rs).
The evaluator captures the interpreter observations, validates the returned
Build configuration, then asks that owner to settle outputs and select generated
sources. It does not assemble durable filesystem evidence itself.

Authored trust grants, component-assumption acceptances, and executed exclusion
receipts are checked by [realization trust](../../realization/trust-model/README.md).
Psi's [wire compatibility owner](../../../psi/interpreter/build-time-evaluation/src/wire_compatibility.rs)
decodes wire demands and checks their meaning. This evaluator sequences those
checks and retains their results; package review reconstructs grants through
the same trust owner without calling the Build evaluator.

Target-declaration preparation, provider-selection decoding and settlement belong to
[provider planning](../../realization/provider-planning/README.md), not this
evaluator. The provider planner retains target-declaration custody and consumes
it into exact typed defaults and origins. Build execution supplies executed
selection receipts to that owner, and package review uses its replay. The integration
tests here exercise that declaration-to-settlement boundary, including canonical
host services and independent-component rejection.

## Source-stage ownership

Source assembly schedules file frontiers. Package compilation's
[source import owner](../../package-manager/compilation-inputs/src/source_imports.rs)
owns project roots, dependency-scope selection, generated-source collision
checks, parsed-file ownership and exact import bindings. Its
[toolchain library owner](../../package-manager/compilation-inputs/src/toolchain_library.rs)
selects and validates the bundled sources.
Psi stage 00 owns physical/in-memory loading, scoped load queues and ordered batch lexing; stage 01 owns
batch parsing into the caller's syntax arena. Their source, token and parsed-root
records live in the existing `source`, `tokens` and `syntax-trees` packages.
Build/Product scope, source coordinates, backing text and diagnostic order travel
through those operations unchanged. The shared
[Build declarations](../../package-manager/build-declarations/src/prelude.rs)
own prelude content; package source preparation selects its injection and scoped
aliases. This coordinator loads and parses the result. Entry-contract seed
selection belongs to the package source owner; neither lexical nor grammar rules
depend on build evaluation. `ProjectSourceSelection` retains exact Build/Product
root scopes and rejoins the selected Build source after parsing. `SourceStorage`
admits the package-root roster, while `admit_dependency_generated_sources` checks
scope-qualified paths before this coordinator loads any generated unit.

## Root-binding implementation boundary

`builder.roots.bind(Target::Slot, Product::entry);` is retained as a dedicated
Psi statement with separate product-context operand paths. It is not an ordinary
machine invocation. The host resolver does not resolve those operands through
host imports; selected-target/product admission still owns that resolution.

Binding follows execution through the original compiler-issued `&mut Build`
cell. Same-package helpers (including imported local modules), reference aliases,
and state transitions use ordinary loan checking. Uncalled helpers and untaken
states contribute no bindings; binding a slot twice on an executed path rejects.
Build.target remains immutable through every alias.

The interpreter returns executed statement coordinates alongside its argument
results, separate from host observations. This owner rejoins them to the exact
prepared program and the occurrence's source/package before target selection.
Requests are not target-admission evidence.

Foreign helpers are admitted lexically: each `roots.bind` occurrence resolves
its implementation against the product machines of its own source package, so a
helper can bind its own package's entry but never the caller's private
declarations. The exact machine symbol survives into Terminal production and
native entry settlement, so same-named machines across packages cannot capture
the selected identity. Owner-selected restricted product-reference handoff to
helpers (no `ProductEntryRef`-style description yet) and computed call-result
receivers remain explicit implementation limits.
[Evaluated build work](../../../../wiki/spec/build/declarations.md#evaluated-build-work)
defines the accepted language; the remaining restricted product-description and
lexical selection handoff is tracked by `BUILD-PRODUCT-REFERENCES` in `TASKS.md`.
