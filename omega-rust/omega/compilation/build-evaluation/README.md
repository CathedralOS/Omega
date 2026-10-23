# Build evaluation

[`build_evaluation.rs`](src/build_evaluation.rs) owns one complete Build activation:
entry selection, pre-effect semantic admission, filesystem preparation, exact machine admission,
restricted-request consent, execution and the returned source commitment.
The coordinator calls `evaluate_build` once and consumes `EvaluatedBuild`; it
cannot obtain an executable checkpoint and skip consent. The private
`admitted_build_program.rs` binds one selected machine to its prepared program;
`execution` consumes that activation against its selected filesystem.
Without a consent guard, a nonempty restricted request returns
`CompileFailure::BuildConsentRequired` before execution, configuration decoding
or generated-source continuation. Checked and dependency Build outcomes retain
those structured requests; package review attributes their canonical meaning
to the exact occurrence and reports dependent findings unavailable. Private
issued snapshot/staging work projects no restricted request and still executes
normally. Current package audit/update commands supply only that private mode;
this stop is also exercised by callers of the public compiler/Build API with
non-private filesystem scopes. It does not add a live-host grant command or
complete the separate recoverable Build-consent/resume workflow.
A prior check may retain its successful Build language judgment for the bound
review pass. Admission uses Psi's exact-input reuse, then freshly settles
comparison execution, filesystem custody, restricted consent and execution;
an unchanged language judgment cannot lend an earlier invocation's grants.
To observe the distinction, set `OMEGA_REVIEW_TIMINGS=1` and run ordinary
`omega update --project <project> --target macos_arm64 --offline` on a
disposable copy of `samples/cli/basics/print_number` with its relative std
dependency preserved. An unchanged bound pass reports `settled_checks=0`;
the operation still checks Build and Product. This is a macOS observation command,
not a cross-host execution claim or a separate review mode.
Without an authored Build entry, it returns default settings and the retained
source commitment without checking, specializing or capturing an execution
snapshot. This shortcut is migration debt: the retained contract requires
compiler-supplied ordinary Build source through the same one-entry evaluation,
dependency join, and freeze route. Final product checking still admits all
ordinary declarations.
For actual Build entries, semantic checking is mandatory even without package
inputs; package declaration authority adds to that floor. Execution consumes the
checker-transformed program and its operator facts, with no separate
specialization pass after checking. Provider derivation and replay use the
explicit Build dependency context and admitted execution profile. Selected
comparison facts follow their exact declaration provenance into the same
pure/granted interpreter; runtime adapter dispatch and service grants do not.
Missing comparison execution custody rejects before Build effects.
The preliminary check still covers the whole provisional program:
removing that duplicate product work requires a sound Build-helper projection
and a separately frozen authored-selection frontier. The interpreter's general
checked-fact dispatch is also unfinished.
[Compilation products](../products/src/evaluated_build.rs)
owns the retained `BuildConfig` and `ComputedBuildConfig` data contracts, re-exported
here for callers. Evaluation produces those records; consumers need not import
the evaluator merely to retain its result.

The [Build-value protocol](../../packages/compilation-inputs/src/build_execution/build_value.rs)
owns exact vocabulary admission, initial Build facets and normalized settings,
including optimization counters and application/privileged-service fields.
One admission object follows initialization through decoding. The compiler
retains the settings whole beside separately checked root/provider/trust rows;
it does not reproduce the value protocol or reconstruct its fields.

Filesystem preparation, retained captured-input views, observation identities and
sealed-output settlement belong to
[package compilation custody](../../packages/compilation-inputs/src/build_execution.rs).
The evaluator captures the interpreter observations, validates the returned
Build configuration, then asks that owner to settle outputs and select generated
sources. It does not assemble durable filesystem evidence itself.

Authored trust grants, component-assumption acceptances, and executed exclusion
receipts are checked by [realization trust](../realization/trust-model/README.md).
Build evaluation decodes wire demands; Psi's
[typed-to-checked stage](../../../psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checking.rs)
checks their meaning before publishing checked trees. Package review reconstructs grants through
the same trust owner without calling the Build evaluator.

Target-declaration preparation, provider-selection decoding and settlement belong to
[provider planning](../realization/provider-planning/README.md), not this
evaluator. The provider planner retains target-declaration custody and consumes
it into exact typed defaults and origins. Build execution supplies executed
selection receipts to that owner, and package review uses its replay. The integration
tests here exercise that declaration-to-settlement boundary, including canonical
host services and independent-component rejection.

## Source-stage ownership

Source assembly is not an evaluator responsibility. Package compilation's
[source assembly](../../packages/compilation-inputs/src/source_assembly.rs)
prepares file frontiers through Psi 00/01. Its
[source import owner](../../packages/compilation-inputs/src/source_imports.rs)
owns project roots, dependency-scope selection, generated-source collision
checks, parsed-file ownership and exact import bindings. Its
[toolchain library owner](../../packages/compilation-inputs/src/toolchain_library.rs)
selects and validates the bundled sources.
Psi stage 00 owns physical/in-memory loading, scoped load queues and ordered batch lexing; stage 01 owns
batch parsing into the caller's syntax arena. Their source, token and parsed-root
records live in the existing `source`, `tokens` and `syntax-trees` packages.
Build/Product scope, source coordinates, backing text and diagnostic order travel
through those operations unchanged. The shared
[Build declarations](../../packages/build-declarations/src/prelude.rs)
own prelude content; package source preparation selects its injection and scoped
aliases. Source assembly loads and parses the result. Entry-contract seed
selection belongs to the package source owner; neither lexical nor grammar rules
depend on build evaluation. `ProjectSourceSelection` retains exact Build/Product
root scopes and rejoins the selected Build source after parsing. `SourceStorage`
admits the package-root roster, while `admit_dependency_generated_sources` checks
scope-qualified paths before source assembly loads any generated unit.

## Pure execution across target requests

One `CompilationSession` retains completed output-free Pure Builds for exact
checked programs, initial values, captured source and dependency custody.
Ordinary `CompileRequest` calls use the per-invocation evaluation policy without
an aggregate sponsor; they may reuse a completed result too. The optional
sponsor's measurement identity and limits participate in matching, so an
unsponsored result cannot satisfy a later limited account. No account or grant
is retained. Each occurrence still performs semantic/authority admission before
looking for a completed result, and every target row remains in that result.
Generated-source and required-output activations remain on their existing path.

A sponsored cache hit pays its recorded logical fuel, BuildLog and result-custody
cost to the current account before publication. Its recorded live demand must
fit beside current reservations. Refusal changes no counters and does not
execute Build again. The returned usage keeps the measured invocation counts
but names current sponsor limits and peaks; the cached producer remains immutable.
This follows [result caching](../../../../wiki/spec/language/evaluation.md#result-caching-and-published-evidence),
so a warm result cannot bypass a ceiling a cold evaluation would exceed.

Run `mbx run -p compiler --example build_session` from the repository root.
This public compiler measurement requires two target products, two preliminary
admissions and one Build execution. It also requires a later one-unit fuel
account to reject and a different source input to execute its own Build.
This is the repeated-work acceptance for `BUILD-EVALUATES-ONCE`, not a new
language fact row or a foreign-host execution claim. The broader capability,
including generated-source and target-row completion, remains open.

Run `mbx run -p compiler --example build_charge` to challenge current accounting.
Two target rows execute one pure Build while paying two logical charges; a
fresh equal-policy account pays its own charge, and an account short by one
fuel unit rejects the warm occurrence without executing it. This measures the
Build workflow rather than adding another language transport fact.

## Root-binding implementation boundary

`Build::bind(builder, Target::Slot, Product::entry);` is retained as a dedicated
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

Foreign helpers are admitted lexically: each `Build::bind` occurrence resolves
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
