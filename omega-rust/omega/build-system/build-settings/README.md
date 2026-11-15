# Build settings

Runs a project's `build.omg` and returns the settings it sets: subsystem,
program entry, provider choices and optimizations. It checks the program before
running it, runs it in the Terminal interpreter, and hands the filled-in `Build`
value back to [orchestration](../orchestration/README.md).

[`build_evaluation.rs`](src/build_evaluation.rs) runs one Build: it selects the
entry, checks the program before any effect, prepares the filesystem, admits the
machine, asks consent for restricted requests and settles the normal return. `PreparedBuildEvaluation` holds that admission
while the application steps one `TerminalBuildHost` activation (one interpreter
run) and joins its dependency requests. Completion accepts
only that run's normal return and produces `EvaluatedBuild`, so the caller
cannot skip consent or substitute another run's result.
`admitted_build_program.rs` binds the selected machine to its prepared program;
`execution` prepares it and settles its returned values.

Without a consent guard, a nonempty restricted request returns
`CompileFailure::BuildConsentRequired` before execution, configuration decoding
or generated sources continue. Checked and dependency Build outcomes keep those
requests; package review attributes each to its exact occurrence and reports
dependent findings unavailable. Private snapshot and staging work issues no
restricted request. Package audit and update use only that private mode; public
compiler/Build API callers with non-private filesystem scopes hit this stop.
No live-host grant command or recoverable consent-and-resume workflow exists yet.

A prior check may keep its Build language verdict for the bound review pass.
Admission reuses Psi's result for the exact same input, then settles comparison
execution, filesystem custody, restricted consent and execution afresh, so an
unchanged verdict cannot lend an earlier run's grants. To see this, set `OMEGA_REVIEW_TIMINGS=1` and run
`omega packages update --project <project> --target macos_arm64 --offline` on a
disposable copy of `samples/cli/basics/print_number`, keeping its relative std
dependency. An unchanged bound pass reports `settled_checks=0` yet still checks
Build and Product. This is a macOS observation, not a claim about other hosts.

Without a `build.omg`, source assembly supplies an ordinary in-memory Build
entry that takes the same route, keeping the admitted source role and owner; it
invents no named declaration and selects no executable entry. An authored Build file must supply its own entry
and project declaration.

Every selected Build entry is checked, even without package inputs; package
declaration authority adds to that minimum. Execution runs the program as the
checker transformed it, with no later specialization pass. Provider derivation and replay use the
explicit Build dependency context and admitted execution profile. Selected
comparison facts keep their declaration origin into the same pure or granted
interpreter; runtime adapter dispatch and service grants do not.
A Build missing comparison execution custody is refused before any effect.

[Deferred constants](src/admission/selected_constants.rs) are prepared and
resumed by this same admission code before it checks the settled program.
Selected dispatch supplies only the exact selected numeric identity; Psi
evaluates and validates its source execution plans. The source continuation is
consumed once, in order, against its original declaration selection and
admitted target context. The first check covers the selected Build source and
its helpers; Product roots wait until the run returns and its requested sources
join.

[Compilation products](../../compilation/products/src/evaluated_build.rs)
defines the `BuildConfig` and `ComputedBuildConfig` records, re-exported here.
Consumers need not import the evaluator to keep its result.

The [Build-value protocol](../../packages/package-compilation/src/build_execution/build_value.rs)
admits the vocabulary and defines the initial Build facets and normalized
settings, including optimization counters and application and privileged-service
fields. One admission object follows the value from initialization through
decoding. The compiler keeps the settings whole beside the separately checked
root, provider and trust rows and never rebuilds their fields.

[`settings.rs`](src/settings.rs) reads the settings out of the original borrowed
Build storage of a Terminal instance that returned normally, then runs the same
validator. It reads paths and selected cases from the instance's own catalog;
the admitted initial value supplies only field labels. Host facets and
product capabilities are not copied. The caller must keep the matching admission
and initial value with the run; this code admits no Terminal artifact as a Build
on its own.
`terminal_entry_writeback` tests a complete Build from source: ordered binding
effects and settings readback across work-budget pauses, with retained and
loaded artifacts, plus nested records, case
payloads, byte views, invalid identifiers, loaders and duplicate optimization
counts. Production Build preparation drives this run through
`activation/preparation.rs` and the Build scheduler. Still to migrate: the
remaining host operations, full accounting of live storage and results, and
suspending for dependencies and providers.

`mbx run -p compiler --example default_build` exercises application, package and
anonymous defaults, one execution per invocation, refusal to substitute a role
or graph, and Product preparation after the live source is deleted. Its Product
defines a trapping machine named `build` and its own `Build` type; neither can
pose as the selected default or the toolchain receiver.
The interpreter's work meter charges the sponsor (the account paying for this
invocation) at each operation and edge. The Build integration test shares one
account across two runs: an exact budget completes both, and one unit less
refuses the second return without publishing settings or replaying effects.
This measures real work, not full storage or result receipts.

[Package compilation custody](../../packages/package-compilation/src/build_execution.rs)
owns filesystem preparation, retained views of captured inputs, observation
identities and settling sealed outputs. The evaluator captures the interpreter's
observations, validates the returned configuration, then asks that code to
settle outputs and select generated sources.

[Realization trust](../../compilation/realization/trust-model/README.md) checks
authored root trust grants and the receipts of executed exclusions. Component
assumptions instead go through the consumer's acceptance and lock file; an
ordinary Build may only narrow that accepted limit, as
[component acceptance](../../../../wiki/spec/build/component_publication.md#consumer-acceptance-and-build-attenuation)
describes. The source call `Build::accept_component_assumption` is gone; Build
evaluation records no component approval. Each verified component's
assumptions become `component_assumption` rows of the root Product occurrence,
and root admission compares them with the consuming project's lock. Build
evaluation decodes the wire demands; Psi checks their meaning before it
publishes the checked program. Package review rebuilds grants through the same
trust code without calling the Build evaluator.

[Provider planning](../../compilation/realization/provider-planning/README.md),
not this evaluator, prepares target declarations, decodes provider selections
and settles them. It keeps the target declarations and turns them into exact
typed defaults and origins. Build execution hands it receipts for the selections
that ran; package review uses its replay. The integration tests here cover that
path, including canonical host services and refusal of independent components.

## Source-stage ownership

The evaluator does not assemble sources. Package compilation's
[source assembly](../../packages/package-compilation/src/source/source_assembly.rs)
loads and parses the files through Psi 00/01. Its
[source imports](../../packages/package-compilation/src/source/source_imports.rs)
own project roots, dependency-scope selection, collision checks on generated
sources, which package owns each parsed file, and exact import bindings. Its
[toolchain library](../../packages/package-compilation/src/source/toolchain_library.rs)
selects and validates the bundled sources.
Psi 00 loads files from disk or memory through scoped queues and lexes batches
in order; Psi 01 parses batches into the caller's syntax arena. Their
source, token and parsed-root records live in the `source`, `tokens` and
`syntax-trees` packages. Build/Product scope, source positions, backing text
and diagnostic order pass through unchanged. The shared
[Build declarations](../../packages/build-declarations/src/prelude.rs)
hold the prelude; package source preparation decides where it is injected and
its scoped aliases, and source assembly loads and parses the result. The package
source code also selects the entry-contract seed; no lexical or grammar rule
depends on build evaluation. `ProjectSourceSelection` keeps the exact Build and
Product root scopes and rejoins the selected Build source after parsing.
`SourceStorage` admits the list of package roots, and
`admit_dependency_generated_sources` checks scope-qualified paths before source
assembly loads any generated unit.

## Pure execution across target requests

One `CompilationSession` keeps each completed Pure Build that wrote no outputs,
keyed by the exact checked program, initial value, captured source and
dependency custody. Ordinary `CompileRequest` calls, which have no shared
sponsor, may reuse a result too. The optional sponsor's identity and limits are part of the match, so a
result computed without a sponsor cannot serve a later limited account. No
account or grant is kept. Each occurrence still passes semantic and authority
admission before it looks for a completed result, and every target row stays in
that result. Builds that generate sources or require outputs are not reused.

A sponsored cache hit pays its recorded logical work, BuildLog and result-custody
cost to the current account before it is published. Its recorded live demand
must fit beside current reservations. A refusal changes no counters and reruns
nothing. The returned usage keeps the measured counts but reports
the current sponsor's limits and peaks; the cached result never changes.
This follows [result caching](../../../../wiki/spec/language/evaluation.md#result-caching-and-published-evidence):
a cached result cannot exceed a limit a fresh evaluation would hit.

`mbx run -p compiler --example build_session` (from the repository root) requires two target products from one Build execution, a later one-unit
work account to be refused, and a different source input to run its own Build.
This is the repeated-work acceptance for `BUILD-EVALUATES-ONCE`, not a claim
about other hosts. Generated sources and completing target rows remain open.

In `mbx run -p compiler --example build_charge`, two target rows run one pure Build and pay two logical charges; a fresh account
with the same policy pays its own charge, and an account one work unit short
refuses the cached occurrence without running it.

## Root-binding implementation boundary

`Build::bind(builder, Target::Slot, Product::entry);` lowers into ordinary
prepare and commit calls at the boundary. Psi keeps the product operands as
source-issued context tied to the artifact and exact call, not as a typed
statement or a run-time lookup by debug name. The host
resolver does not resolve those operands through host imports; admission of
the selected target and product still does.

Binding follows execution through the original `&mut Build` cell the compiler
issued. Same-package helpers (including imported local modules), reference
aliases and state transitions use ordinary loan checking. Uncalled helpers and
unentered states add no bindings; binding a slot twice on a path that runs is
refused. Build.target stays immutable through every alias.

The interpreter returns executed lexical selections apart from host
observations. Its binding adapter authenticates the original
receiver, issues a ticket valid for this run and consumes it once at commit.
This code rejoins each request to its occurrence's source and package before
target selection; a request is not evidence for target admission.

Crash, physical-class and service exclusions use the same prepare and commit
calls. Checked Omega helpers encode evaluated enum values; the host neither
reads source expressions nor guesses enum layout tags. The record of the normal
return keeps the authored call and the machine that selected it. Final Product
admission resolves service declarations and exact toolchain cases, then applies
the restrictions to each selected target's closure. Formation sees every
service operand; only execution selects a restriction. Duplicate restrictions
combine, and a branch not taken imposes none.

`mbx nextest run -p compiler --test terminal_behavior_exclusions` checks computed
helper values, receiver and ticket custody, per-target refusal, exact service
identity, physical classification, and native and reloaded-Terminal execution.

Helpers from other packages are admitted lexically: each `Build::bind`
occurrence resolves its implementation against the product machines of its own
source package. A helper can bind its own package's entry but never the
caller's private declarations. The exact machine symbol survives into Terminal
production and native entry settlement, so machines with the same name in
different packages cannot take over the selected identity. Two limits remain:
handing an owner-chosen restricted product reference to a helper (no
`ProductEntryRef`-style description yet), and receivers computed by a call.
[Evaluated build work](../../../../wiki/spec/build/declarations.md#evaluated-build-work)
defines the accepted language; the remaining restricted product-description and
lexical selection handoff is tracked by `BUILD-PRODUCT-REFERENCES` in `TASKS.md`.

Native imports owned by the Build use the same prepare and commit calls on the
original run, in [native_bindings.rs](src/host/bindings/native_bindings.rs).
The checked, owned `ForeignBinding` value is decoded through Psi's common core
vocabulary; this host records requirement, target and locator as inert intent,
plus its accounting. It neither selects a provider nor calls the
foreign code. Checked source formations travel beside their exact Terminal
artifact before Build bodies are erased. Normal completion keeps those
formations, reached effects, code identity and observations for package output.
