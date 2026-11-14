# Build settings

Runs a project's `build.omg` and returns the settings it sets: subsystem,
program entry, provider choices and optimizations. It checks the program before
running it, runs it in the Terminal interpreter, and hands the filled-in `Build`
value back to [orchestration](../orchestration/README.md).

[`build_evaluation.rs`](src/build_evaluation.rs) owns one complete Build activation:
entry selection, pre-effect semantic admission, filesystem preparation, exact machine admission,
restricted-request consent and normal-return settlement.
`PreparedBuildEvaluation` retains that admission while the application advances
one `TerminalBuildHost` activation and joins its dependency requests. Completion
accepts only that invocation's normal return and produces `EvaluatedBuild`; the
caller cannot skip consent or substitute another invocation's result.
`admitted_build_program.rs` binds the selected machine to its prepared program;
`execution` prepares it and settles its returned values.
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
`omega packages update --project <project> --target macos_arm64 --offline` on a
disposable copy of `samples/cli/basics/print_number` with its relative std
dependency preserved. An unchanged bound pass reports `settled_checks=0`;
the operation still checks Build and Product. This is a macOS observation command,
not a cross-host execution claim or a separate review mode.
Without a physical `build.omg`, source assembly supplies an ordinary in-memory
Build entry. It follows the same checking, Terminal activation, dependency join
and freeze route, retaining the admitted source role and owner without inventing
a named declaration or selecting an executable entry. An authored Build file
must supply its own entry and project declaration.
For every selected Build entry, semantic checking is mandatory even without package
inputs; package declaration authority adds to that floor. Execution consumes the
checker-transformed program and its operator facts, with no separate
specialization pass after checking. Provider derivation and replay use the
explicit Build dependency context and admitted execution profile. Selected
comparison facts follow their exact declaration provenance into the same
pure/granted interpreter; runtime adapter dispatch and service grants do not.
Missing comparison execution custody rejects before Build effects.
[Deferred constants](src/admission/selected_constants.rs) are prepared and resumed
by this same admission owner before checking its settled program. Selected dispatch
supplies only the exact selected numeric identity; Psi performs evaluation and
validates its source execution plans. The source continuation is consumed once,
in order, against its original declaration selection and admitted target context.
The preliminary check covers the selected Build source and its helpers;
Product roots remain deferred until the activation returns and its requested
sources join.
[Compilation products](../../compilation/products/src/evaluated_build.rs)
owns the retained `BuildConfig` and `ComputedBuildConfig` data contracts, re-exported
here for callers. Evaluation produces those records; consumers need not import
the evaluator merely to retain its result.

The [Build-value protocol](../../packages/package-compilation/src/build_execution/build_value.rs)
owns exact vocabulary admission, initial Build facets and normalized settings,
including optimization counters and application/privileged-service fields.
One admission object follows initialization through decoding. The compiler
retains the settings whole beside separately checked root/provider/trust rows;
it does not reproduce the value protocol or reconstruct its fields.

[`terminal_settings.rs`](src/terminal_settings.rs) projects the settings from a
normally returned Terminal instance's original borrowed Build storage, then
uses that same validator. It reads paths and selected cases from the instance's
own catalog; the admitted initial value supplies only result-descriptor labels.
Host facets and product capabilities are not copied into settings. The embedding
must retain the matching admission and initial value with its activation; this
projection does not independently admit an arbitrary Terminal artifact as Build.
`terminal_entry_writeback` exercises complete source-produced Build execution,
ordered binding effects and settings readback across work-budget pauses, using both
retained and loaded artifacts. It also tests nested records, case payloads,
byte views and invalid identifiers, loaders and duplicate optimization counts.
Production Build preparation uses this Terminal activation through
`activation/preparation.rs` and the Build scheduler. With no `build.omg`, source
assembly admits an ordinary in-memory default and keeps Product roots deferred
until its checked activation returns. Its private source selection preserves
supplied role/owner and cannot name a new project or bind an executable entry.
Remaining host operations, complete live-storage/result accounting and
dependency/provider suspension retain migration work.

`mbx run -p compiler --example default_build` exercises application, package and
anonymous defaults, one execution per invocation, role/graph substitution refusal,
and retained Product preparation after deleting live source. Its Product defines
a trapping machine named `build` and its own `Build` type; neither can impersonate
the selected default or the toolchain receiver.
Terminal's existing work meter now accepts the exact invocation sponsor and
charges it at the ordinary operation/edge sites. The Build integration witness
uses a shared account across two activations: an exact budget completes both,
one unit less refuses the second return without publishing settings or replaying
effects. These are real work observations, not complete storage/result receipts.

Filesystem preparation, retained captured-input views, observation identities and
sealed-output settlement belong to
[package compilation custody](../../packages/package-compilation/src/build_execution.rs).
The evaluator captures the interpreter observations, validates the returned
Build configuration, then asks that owner to settle outputs and select generated
sources. It does not assemble durable filesystem evidence itself.

Authored root trust grants and executed exclusion receipts are checked by
[realization trust](../../compilation/realization/trust-model/README.md).
Component assumptions instead use the existing consumer acceptance/lock route;
ordinary Build policy may only attenuate that accepted ceiling under
[component acceptance](../../../../wiki/spec/build/component_publication.md#consumer-acceptance-and-build-attenuation).
The source `Build::accept_component_assumption` approval call is removed:
Build evaluation records no component approval. Each verified component's
assumptions become `component_assumption` rows of the root Product occurrence,
and root admission compares them with the consuming project's lock.
Build evaluation decodes wire demands; Psi's
[typed-to-checked stage](../../../psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checking.rs)
checks their meaning before publishing checked trees. Package review reconstructs grants through
the same trust owner without calling the Build evaluator.

Target-declaration preparation, provider-selection decoding and settlement belong to
[provider planning](../../compilation/realization/provider-planning/README.md), not this
evaluator. The provider planner retains target-declaration custody and consumes
it into exact typed defaults and origins. Build execution supplies executed
selection receipts to that owner, and package review uses its replay. The integration
tests here exercise that declaration-to-settlement boundary, including canonical
host services and independent-component rejection.

## Source-stage ownership

Source assembly is not an evaluator responsibility. Package compilation's
[source assembly](../../packages/package-compilation/src/source/source_assembly.rs)
prepares file frontiers through Psi 00/01. Its
[source import owner](../../packages/package-compilation/src/source/source_imports.rs)
owns project roots, dependency-scope selection, generated-source collision
checks, parsed-file ownership and exact import bindings. Its
[toolchain library owner](../../packages/package-compilation/src/source/toolchain_library.rs)
selects and validates the bundled sources.
Psi 00 owns physical/in-memory loading, scoped load queues and ordered batch lexing; Psi 01 owns
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

A sponsored cache hit pays its recorded logical work, BuildLog and result-custody
cost to the current account before publication. Its recorded live demand must
fit beside current reservations. Refusal changes no counters and does not
execute Build again. The returned usage keeps the measured invocation counts
but names current sponsor limits and peaks; the cached producer remains immutable.
This follows [result caching](../../../../wiki/spec/language/evaluation.md#result-caching-and-published-evidence),
so a warm result cannot bypass a ceiling a cold evaluation would exceed.

Run `mbx run -p compiler --example build_session` from the repository root.
This public compiler measurement requires two target products and one Build
execution. It also requires a later one-unit work account to reject and a
different source input to execute its own Build.
This is the repeated-work acceptance for `BUILD-EVALUATES-ONCE`, not a new
language fact row or a foreign-host execution claim. The broader capability,
including generated-source and target-row completion, remains open.

Run `mbx run -p compiler --example build_charge` to challenge current accounting.
Two target rows execute one pure Build while paying two logical charges; a
fresh equal-policy account pays its own charge, and an account short by one
work unit rejects the warm occurrence without executing it. This measures the
Build workflow rather than adding another language transport fact.

## Root-binding implementation boundary

`Build::bind(builder, Target::Slot, Product::entry);` lowers into ordinary
boundary prepare/commit calls. Psi retains its designated product operands as
source-issued companion context bound to the artifact and exact call identities,
not as another typed statement or runtime debug-name lookup. The host resolver
does not resolve those operands through host imports; selected-target/product
admission still owns that resolution.

Binding follows execution through the original compiler-issued `&mut Build`
cell. Same-package helpers (including imported local modules), reference aliases,
and state transitions use ordinary loan checking. Uncalled helpers and untaken
states contribute no bindings; binding a slot twice on an executed path rejects.
Build.target remains immutable through every alias.

The interpreter returns executed lexical selections separately from host
observations. Terminal's binding adapter authenticates the original receiver,
issues an activation-local ticket and consumes it once at commit. This owner
rejoins requests to the occurrence's source/package before target selection;
requests are not target-admission evidence.

Crash, physical-class and service exclusions use this same prepare/commit
protocol. Checked Omega helpers encode evaluated enum values; the host neither
reads source expressions nor guesses enum layout tags. The normal-return ledger
retains the authored call and selecting machine. Final Product admission resolves
service declarations and exact toolchain cases before applying the restrictions
to each selected target closure. Formation sees every service operand; execution
alone selects a restriction. Duplicate restrictions union, and an untaken branch
does not impose one.

`mbx nextest run -p compiler --test terminal_behavior_exclusions` checks computed
helper values, original receiver/ticket custody, target-local refusal, exact
service identity, physical classification, and native/reloaded-Terminal execution.

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

Build-owned native imports use the same original-activation prepare/commit
protocol in [terminal_bindings/native_bindings.rs](src/terminal_bindings/native_bindings.rs).
The checked, owned `ForeignBinding` value is decoded through Psi's common core
vocabulary; this host records inert requirement/target/locator intent and its
actual accounting. It neither selects a provider nor calls the foreign code.
Checked source formations travel beside their exact Terminal artifact before
Build bodies are erased. Normal completion retains those formations, reached
effects, code identity and observations for the package-output owner.
