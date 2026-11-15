# Compiler coordination

This crate calls the compiler's stages in order; it does not load packages,
evaluate builds, transform programs or draw visualizations. [lib.rs](src/lib.rs)
offers two calls for any number of targets: opaque source preparation, then
completion returning exactly what was asked. Application
[`compile_sources`](../../build-system/orchestration/src/compile/source_compilation.rs)
owns each single-use `CompileRequest` and runs Build between them. See the
[pipeline map](../../../pipeline.md).

[Terminal production](../../../psi/compilation/terminal-production/README.md)
publishes the module the lowered-form checker accepted, with its evidence and
source owner. This crate supplies product policy and timings;
[entry receipts](../../../psi/compilation/representations/terminal-psi/codec/src/entry_receipt.rs)
belong to the codec. A product the compiler cannot yet produce is refused with
`no product route yet`; no old lowering sequence runs.

The separate [native compiler](../native-compiler/README.md) turns Terminal Psi
into an image without reading source; this crate calls it after Terminal
production. Code that needs those contracts uses `native-artifact`, `image-emission`, and Omega 00's `provider_admission`
directly. Reports and publication come from `products`; this crate has no
`report` or `artifact_publication` module. Root-level request/result exports
are conveniences for the entry points.

[target_batch.rs](src/target_batch.rs) admits the request and prepares one
pending source/Build prefix; completion waits for its result, then dispatches
the Check, Terminal or Native product, settling every target before producing
Terminal Psi. Targets with the same checked source owner and
the same Psi optimization share one Terminal production and its storage; each
target keeps its own receipts and exclusion judgments. Native production
receives the artifact explicitly and never calls Psi. All targets' roots and
callbacks go into one shared list of Terminal entries.

The crate-internal `produce_products` goes from checked source and selected
targets to Terminal Psi when needed, then to the requested outputs. It is not
a public way to compile an existing Terminal artifact; that source-free route is
[`native_compiler::prepare`](../native-compiler/src/preparation.rs), then
scalar completion, then
[`native_compiler::compile`](../native-compiler/src/compile.rs). Preparation
keeps the Terminal artifact and explicit realization inputs; compilation
consumes that work once its scalars are complete. Neither calls an interpreter
or reopens source.

`cargo run -p compiler --example terminal_sharing` compiles two native targets
this way and checks real production counts and shared storage; it runs no
foreign-host image. `--check` and `--terminal` stop at the other report points.
To check that one target's failure spares the others, use `--reject linux_arm64
tests/fixtures/pass/targets/shared_program_distinct_roots_exit/main.omg
windows_x86_64 linux_arm64 macos_arm64`: the target missing its entry is
rejected and both others are kept.

Product constants and declaration conditions travel in the shared Terminal
artifact, not a per-target program copy. Scalar completion keeps exact
provider and target inputs and evaluation charges. `--check`
publishes declarations without an entry root when scalar values need
completing, and otherwise stops before Terminal production.
`--compare-single-targets --run` also compares shared-product bytes with
per-target compilation and runs the host image and reloaded Terminal artifact. [Package compilation](src/checked/package_compilation.rs)
builds helpers from the same checked source without private constant replay. The waiting Build activation admits each slot, completes
scalars and installs components in
[provider installation](../../build-system/build-settings/src/activation/provider_installation.rs).
Its live account pays for evaluation; installed components keep completed
scalar inputs for the rest of the invocation.

The coordinator keeps one ordered list of completed outputs, errors and
pending Terminal work, lends only the pending work to shared production, then
finishes each application in order, with no indexed result slots or parallel
tables. [Product execution](src/product_execution.rs)
owns one application's stopping point, permission admission, artifact
completion and output.
[Product report planning](../products/src/checked_compilation_report.rs) owns
the requested output and native production inputs and settles optimization
rollback once; Terminal, native production and the final receipt use those
choices. A native request is carried by its native report settlement, not
compared afterward with a separate requested-mode tag.

The [session](src/checked/compilation_session.rs) owns the shared source/Build
continuation and retained checking. Provider selection still runs before
Terminal production, which needs the selected entry and callback roots and
exclusion inputs; installation, permission comparison and trust admission run
after it. Moving provider handoff after Terminal Psi is separate, unfinished
work.
The [product request owner](../products/src/compile_request.rs) expands
explicit targets under one invocation policy, rejects duplicates and
incompatible rollback, and admits evaluated Build requirements. It returns
immutable per-target inputs; the coordinator runs the loop, not those
decisions. The coordinator assembles and resolves source, keeps the Build
continuation through application execution, then types authored and
generated code once.
[Prepared source products](../products/src/prepared_build.rs) carry that
checked prefix and sealed source custody, not a cache keyed by target inputs.
Provider settlement borrows them and cannot change checked metadata.
Package review uses the same operations with its own admitted activation. Its
compiler session shares checked storage, and package targets finish through
the common Terminal batch; its source completion still has to be merged with
the whole-program request. Native production consumes each target's retained
Terminal artifact without scheduling it again.

Application Build asks for package admission, then schedules
`PackageCompilationRequest`s in dependency order in
[`compile_packages`](../../build-system/orchestration/src/package/package_compilation.rs),
with the requested product and root permission consent given up front. Its
[Build execution owner](../../build-system/orchestration/src/compile/build_execution.rs)
walks the dependencies, joins generated outputs and waits for demanded
independent components before their consumers continue; the compiler does not
schedule this. The compiler's
[package facade](src/checked/package_compilation.rs) prepares one admitted
Build, turns the returned activation into opaque `PackageBuild` custody, and
completes ready component and product batches. Application code cannot extract
the source continuation; component executables stay in this session.
Package management supplies custody, consent and staging, not compiler calls
or continuation tables.

Independent-component requests, including lazy review descriptions, use the
same [product sequence](src/target_batch.rs) as root products. The
[component owner](src/product_execution.rs) prepares existing checked custody
and, after shared Terminal production, completes the description and
permission evidence; it has no batch loop or compiler entry of its own.
[Product batching](src/checked/product_batch.rs) groups the admitted root and
component continuations by complete source input and checks each group once.
Members borrow its checked program and sealed source custody but keep their own
Build receipts, charges, timings and target settlement.
Request identity and product policy travel with each continuation; component
destinations travel with their checked products. Batches return results in
authored order, without parallel arrays or empty slots. Component publication
lends ready work to shared Terminal production and keeps refusals in the same
list.

Each admitted application prepares candidate target-provider declarations
before Terminal production; the emitted artifact then tells the ordinary
provider selector what is demanded, without reopening typed source. Unused
alternatives stay unselected, family completeness is checked against the
selected list, and failures of other targets remain proposal rows. Sharing
Terminal storage never lets one application use another's Build defaults,
overrides or assumed bindings.
This grouping covers only the submitted batch, not earlier work. Grouped
continuations must hold the same retained evaluation authority; matching paths
or settings are not enough.

`mbx run -p orchestration --example package_batch` counts one whole
invocation's work through application Build. Counting starts before
local-project preparation and is checked even when production rejects. The
default two-target fixture must take one Build execution, two settled checks
(Build and Product), one source completion and one shared Terminal allocation;
`--check` keeps those counts and produces no Terminal program. Each use admits
its own Build effects and charges; only exactly matching Pure Builds reuse
computation. Components demanded in
different dependency rounds do not share completion through this batch.

The ordinary dependency example keeps two different Builds and one Product
check instead of merging them:

```sh
mbx run -p orchestration --example package_batch -- tests/fixtures/pass/build/package_batch_distinct_builds/main.omg 1 1 2 3
mbx run -p orchestration --example package_batch -- --check --targets linux_x86_64 tests/fixtures/fail/build/unrequested_target_entry_returns_value/main.omg 1 0 1 2
```

The second command must reject `foreign_launch` because `ProgramEntry` has no
result, even though its Windows row was not requested. Expected counts come
from the source, never from the observed counters; expecting two Builds from
the default equal-source fixture must fail. Counts are process-wide, so run
each measurement alone.

`mbx run -p compiler --example package_batch_sources` submits three products
from two real source roots through application Build, mixed with a rejected
empty package plan that stays in position without affecting them. The two
equal-source consumers share one checked and Terminal group; the other root
gets its own. Project discovery supplies the two Builds. The whole measurement, discovery included,
must take two Builds, four checks, two source completions and two Terminal
productions, sharing storage only for the equal-source pair; product
preparation runs no further Builds. Missing retained custody and a late-added
required output are both rejected before any work. The example uses
package-owned source resolution and Product admission, and its built-in entry
bindings need no fixture acceptance. These measurements advance
`BUILD-EVALUATES-ONCE` but do not finish its remaining per-target work. The
request runs on one compiler-stack worker; Builds and source checks start no
more workers.

Fresh execution and retained-source Product inputs enter the same
`orchestration::compile_packages` scheduler as `PackageSourceRequest`
variants; `prepare_package` uses the same inputs for review consumers that stop
before Product checking. A retained request missing its source custody or
pre-effect contract never falls back to running the Build.

Ordinary package operations return admitted requests and keep custody to
reconcile results; they do not call compilation. Application Build receives
reports or Terminal inspections, never a checked checkpoint to hand back.
Before native production the compiler compares freshly supplied permissions
with the root Product's exact consent, through the review-evidence projection
owner. Inspection is an explicit product with no
native authority. Standalone inspection and corpus production submit the same
`CompilationProduct` through `orchestration::produce_source`. Named inspection
joins shared Terminal production and completes declaration conditions against
that artifact, with no throwaway check artifact first. Same-name requests share
code and exact Build companions; different inspected roots get separate
products. Source checking returns selected provider custody internally. Both
standalone and package products finish source-side provider preparation before
Terminal, then admit receiving permissions and complete artifact proposals;
there is no late source installation phase or second route from checked source
to product. Check-only and artifact-only requests stop where requested; a
Check with scalar values still produces its rootless declaration artifact, and
artifact-only requests emit no native images. Review descriptions borrow the
same Terminal production inputs; the producer ignores review history.

Check-only consumers pass a `CheckedCompileRequest` to
`orchestration::check_source`. Package inputs, build staging and session
sponsors are request data, not separate entry points. The request enters the
same prepared source continuation as production and target batches. Ordinary production takes one `CompileRequest::with_build_snapshot`
inventory and list of required outputs. `CompileRequest::for_targets` takes the
package root, one Build staging directory and the selected profiles; artifact
publication decides where outputs go. A Check with no target uses one absent
profile. Different caller inventories are different invocations, not target
variants of one compilation. Each target admits the supplied custody itself,
and one target's failure never hides another's.

Package [compilation reuse](../../packages/package-compilation/src/reuse/compilation_reuse.rs)
compares sources: assembly records imported content and entry bindings. There
is no per-target Build result cache or live-output reuse key. Build uses its
invocation's single staging account, and its captured outputs can be published
beside several target products without rerunning it. Source-reuse keys
never grant Build authority. With no request, canonical package inputs are
still captured automatically. Standalone scoped inventories must include the
source files they consume and do not expose undeclared siblings. Without a
caller-supplied sponsor, captured builds use fresh private output staging,
released once the retained bytes are captured. Artifact-only builds return
completed files without needing a program entry; executable builds use the
same file carrier for companions. Publication writes only completed files
under `completed/<set-identity>/files/`, with `manifest.bin` recording the
exact build observation, logical paths, lengths and byte digests. An existing
set is checked against retained bytes, not trusted by directory name. Scratch
is never published; a failed compilation returns no successful set. See
[scoped build inputs](../../../../wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem).

The [checked entrance](src/checked/checking.rs) shows the steps in order:
[evaluate Build and continue generated source](src/checked/checking/build_continuation.rs),
[check Product and seal source custody](src/checked/checking/source_checking.rs),
then [prepare each target's complete selected execution](src/execution_settlement.rs).
The Build coordinator calls the [Psi source continuation](../../../psi/compilation/source-compiler/src/source_compilation.rs)
for resolution, typing and generated-source completion, timing those calls
directly. It owns package admission and Build authority, not the
resolver/typing fixed points or their arenas.
[Psi source checking](../../../psi/compilation/source-compiler/src/source_checking.rs)
then owns preparation, matching retained judgments, checking and product
pruning. Build admission uses the same checking steps. Shared source
projections and operator queries belong to the semantic checking libraries.
The product owner keeps the complete checked result, evaluated Build and
source custody, and owns their metadata projections. Its shared Build records
are plain data for evaluation and product assembly and import neither the
evaluator nor the coordinator. Compilation schedules the stages visibly and
hands their complete results to sealing.

The [Build evaluator](../../build-system/build-settings/src/build_evaluation.rs)
owns the whole pre-effect protocol (admission, consent, execution) and returns
the configuration, restricted requests and admitted source commitment together.
Its private declaration admission still uses the preliminary Psi checked pass;
removing that duplicate is a separate task. No compiler-private package gate or
public raw Build checkpoint can bypass this protocol. The result keeps the
selected-execution settlement whole, and the evaluated Build is one shared
immutable product, not split into parallel fields for configuration,
observations, usage, optimization report intent and generated outputs.
Generated-source bundles project the same retained outputs; source custody
only adds where they sit in the source. Build consent runs before execution and
the selected-machine check after it.
Restricted host-request types and their normalization belong to package [build-execution custody](../../packages/package-compilation/src/build_execution/restricted_requests.rs),
beside the filesystem grants, captured inventories and sponsor limits they
describe. Build evaluation checks consent under the admitted execution mode
before running. Review and encoding use that package contract directly.
Authored Build policy stays separate from per-target realized evidence, and
two checked products are equal only if their complete evaluated Builds are.
Clones share program storage until changed; review instantiation borrows table
rows instead of rebuilding scratch from clones. A change never reseals
evidence; later stages must rebuild it.

Diagnostic tools call `orchestration::check_source(session, request)` for the
verdict and independent-component discovery. The session owns source
frontiers, Pure Build values, language judgments and Product root selections,
so callers keep no caches. Root paths and immutable package inputs
must match, and every activation admits current custody, grants, selections
and sponsor charges. Evaluated selections survive a later settlement rejection,
so review can supply component descriptions and account for the rejected
attempt. Successful language judgments share checked storage, not authority;
all reuse stays within one invocation. Package targets finish together through
`complete_package_products`, so equal checked programs produce Terminal Psi
once before native realization.

[Product completion](src/product_execution.rs) finishes the retained Terminal
application and passes explicit component, permission and image inputs to
[`native_compiler::compile`](../native-compiler/src/compile.rs). That
source-free entry point emits components and the root, admits exact receiver
calls and keeps component runtimes; product coordination does none of this.
Fresh compilation and package re-entry share this route: there is no native
driver that reads source, no second program-entry artifact and no
prepared-compilation cache. Package acceptance is recorded apart from the
caller's optional receiving policy; neither stands in for the other. The
[product contract](../../../../wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission)
needs no receiving policy just to emit an artifact; explicit ecosystem
admission still checks the policy it is given. `CompileOutcomes` returns one
`CompileReport` or diagnostic result per target and hides no failed target.
Callers wanting one product use `into_single_report`, which rejects any other
count. Publishing an executable comes later.

[Compilation products](../products/README.md) owns publication:
`publish_compilation(report, directory)` picks the retained Native, Terminal
or build-only product, installs its requested proof pairs, then publishes the
completed build companions; the CLI and library callers both use it. Its
`CompileReport` owns payload count and receipt joins, not filesystem work;
none of this lives in this crate. Check-only and already-published reports are
rejected before the filesystem is touched. Backend artifact owners still build native evidence; platform
custody owns staging and installation.

## Supporting packages

Build activation belongs to
[build-settings](../../build-system/build-settings/README.md), a separate
package used by the coordinator and package review. Project declaration
grammar, retained build outputs and compilation-input custody belong to the
package system's
[declarations](../../packages/build-declarations/README.md),
[outputs](../../packages/build-output/README.md) and
[inputs](../../packages/package-compilation/README.md) packages; consumers
depend on them directly, not on the package-command application.

Target-neutral build evaluation stays in
[Psi's compiler support](../../../psi/compilation/build-time-evaluation/README.md).
Opaque representation admission belongs to
[Psi checking](../../../psi/compilation/validation/representation-planning/README.md);
provider selection and planning belong to
[Omega provider realization](../realization/provider-planning/README.md), beside
[selected execution](../realization/selected-dispatch/README.md).
[Realization admission](../realization/trust-model/README.md) rebuilds trust
obligations and reports. The [artifact utilities](../../foundation/artifacts/README.md)
write host output and take opt-in measurements; they keep no semantic trust
records. The separate [package manager](../../packages/package-manager/README.md)
installs and updates packages.

## Product boundaries and observations

The [Terminal coordinator](src/terminal/terminal_artifact.rs) admits
composition and the exact entry selection, then asks the Psi facade for one
shared program and an entry receipt per application. Psi admits authored
exclusions before optimizing. Native permission admission and proposal
construction follow production without restarting Psi or re-verifying its
output. Callback placements stay in Omega, not in Psi as an opaque payload. One Psi optimization selection covers the whole program,
callback roots included. The coordinator passes the artifact and build choices
to the [shared native proposal owner](../representations/abstract-operations/verified-optimization-input/src/native_proposal.rs),
which checks the entry, selected implementations and callback occurrences. Its
[execution preparation](../representations/abstract-operations/verified-optimization-input/src/native_execution.rs)
then finds the functions that can run and completes their implementation
choices, and [Omega 00](../pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal.rs)
lowers and admits that checked selection. The compiler calls these steps
without rebuilding their joins, and later stages read the shared record
without importing its producer. A missing callback body is rejected during
shared production or the declaration join, without running an artifact.

For authored exclusions, Terminal production borrows the unoptimized program
and checks each application's main and selected callback roots before
optimization, using the backend effects checker. On rejection the retained
machine provenance supplies machine names and declaration spans; that mapping
is for diagnostics only, not admission evidence.

[Request validation](../products/src/compile_request.rs) checks the requested
product and rollback before reading source or opening output directories.
Checking allows no rollback, Terminal publication allows only the Psi-stage
rollback that applies, and native products allow their applicable phases.
Rollback only removes optimizations; it never adds any or picks a fallback
backend.

A retained native-artifact request runs the checked frontend and the Terminal
producer, then the same source-free [native realization](#native-realization)
that component staging uses. The non-clonable result keeps the Terminal
identity, target, selected providers, object and relocation evidence, encoded
text and separately replayed image. It writes no primary output
and grants no authority to an executable path, installation, publication,
deployment or runtime. Pending component work cannot be silently dropped.
Unsupported Terminal Psi is rejected; no source-based route takes over.
[Native artifacts](../backend/artifacts/native-artifact/README.md) lists the
current limits on physical evidence.

Compilation produces only the requested product and diagnostics: no optional
JSON, HTML, text or Markdown debug dumps, disassembly reports or timing files,
and no full/output-only observation policy.

[Trust admission](../realization/trust-model/README.md) still rebuilds trust
obligations, settles exact owner admissions and checks that target and report
agree. Wire compatibility, capability validation, package locks and required
product evidence remain mandatory. A check or retained-artifact compile need
not create a build directory; explicit build effects and primary publication
have their own filesystem needs.

The CLI's `--timings` option prints preparation, compilation and native
publication times (where they apply) and the total on stderr. These are
command steps, not a profile of internal passes. A failed command still
reports its total. Normal compilation leaves timing off and does
not install an allocation-counting global allocator.

Source-consumption hashing reuses one row buffer and the package owner's row
encoder, so the byte format and identities do not change.

## Intrinsic settlement conversion

[Native builtin projection](../realization/provider-admission/src/compiler_builtins.rs)
checks the provenance of every selected plan, then joins the sorted intrinsic
rows to what the Terminal program demands, keeping each row's original plan
index. Duplicate demanded rows are rejected; undemanded rows need no
executable proposal. One exhaustive mapping converts planner classifications
into the native catalog and rejects unsupported executions, as the
[boundary contract](../../../../wiki/spec/terminal-psi/boundary_calls.md)
requires. It reads source-free plan and execution-identity views, so provider
planning does not depend on target-operation IR. Native admission separately
rejoins the proposals to the receiving target's catalog.

## Target set and realization

[Explicit targets](../backend/target/src/explicit_target_set.rs) takes an
optional selection. Without one, every deployable profile the toolchain
provides is realized. A given set must be nonempty; it is sorted and
deduplicated, and wildcards and unknown targets are rejected. The selection only narrows realization: Psi
checks every target's bodies once per compilation either way
([multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation)).

[Request admission](../products/src/compile_request.rs) stores the root,
product, observation policy, package sources and target set once. Compilation
collects one ordered outcome per realized target without stopping at the
first failure: a shared checking failure gives every target the same
diagnostics, and one target's failure never hides another's. A success keeps
its ordinary standalone artifact identity. The collection claims no support,
test or audit status.

The current implementation instead keeps one immutable source parse
([package source preparation](../../packages/package-compilation/src/source/source_assembly.rs))
and runs a child compilation per target configuration: per-target source
assembly, declaration filtering, build evaluation, provider settlement and
checking, with request-level policy copied into each.
[Generated-source continuation](../../../psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/README.md)
owns appended source, and [checked settlement](#checked-settlement) owns the
later ordered joins. The [pipeline route items](../../../../TASKS.md#pipeline-route)
replace the child compilations with the single pass above. Until then each
child prepares its own Terminal input, and sharing the parse never lets one
child's checked target, providers, admissions or physical evidence serve as
another child's authority.

## Checked settlement

[Package source preparation](../../packages/package-compilation/src/source/source_assembly.rs)
parses all source: physical files and code generated by dependencies or
activations. The coordinator passes the syntax trees to resolution and typing
and does no loading, lexing or parsing itself. Corpus discovery runs Build
through the application owner and reads its retained entry bindings, with no
parser or static dependency/target view of its own.

Psi owns [source checking](../../../../wiki/spec/language/state_contracts.md),
including exact boundary-call and source-authority checks; the compiler issues
no separate source-provider approval afterward. The compiler forwards
independently derived opaque-representation property receipts, which Psi
rejoins to their authored dependency-scope instances while preparing input.
The coordinator gives Psi's checking request the evaluated build authority,
and Psi checks assembly authority even when reusing a memoized language
judgment. Language-only probes grant no runtime installation
authority.

[Source checking](src/checked/checking/source_checking.rs) uses Product typing
once, admits representation properties and seals source custody before target
settlement; targets share the checked arenas and the sealed custody.
[Provider binding](../realization/selected-dispatch/src/provider_binding.rs)
belongs to Omega realization. It binds callback placements and provider
grants, closes selected review and accepted service roles, and builds
direct-call supplies without rewriting Psi's checked result. Psi derives
candidate specializations from authored conformances, independently of runtime
provider selection. On the lowered route, `SelectedExecution` keeps only the
target and its settled provider plans
([selected-dispatch](../realization/selected-dispatch/README.md)). Terminal
production borrows it to build the provider proposal a target's conditions
are checked against. Turning it into a checked compilation only assembles
custody, not a late source phase.

Native proposal construction replaces checked-adapter specialization rows with
Terminal Psi's complete candidate associations. It uses the same join (exact
selecting owner, package, template and closed coordinates) as target-provider
publication, and unused inherited instances stay in the list. Native authority
review, callback settlement and emitted evidence use this normalized
selection. Private native-input records omit the source-issued checked
specialization rows; reloading rebuilds them from Terminal Psi. Source checking
and review keep their own source-side facts; external applications, intrinsic
leaves and installation reach are held separately.

Provider-dependent Product constants are currently rejected with an explicit
`unimplemented:` diagnostic. Portable symbolic recipes must pass
through Psi before realization closes them; selected runtime values cannot be
folded into shared source. Admitted Build evaluation keeps its own evaluation
authority. Target-neutral execution plans belong to Psi: one Omega authority
pass joins each source requirement to an exact intrinsic or checked adapter
and its plan commitment. Psi derives the Float operation and carriers or the
provider entry, then itself validates source selection, operands, arithmetic
policy, signature, conformance and invocation admission. The coordinator
neither classifies Float meaning nor rebuilds separate fact lists for these
two supplies.

[Selected execution](../realization/selected-dispatch/src/selected_execution.rs)
keeps Psi's checked execution plans instead of calling Psi again to rebuild
them. Check-only task validation stays separate from native task demand read
from the artifact. When both exist, source and role errors come before
Terminal and task errors; native-source refusals come later, and native
permission and trust admission still run after production. This remains a
seam between source and realization; moving it out of the coordinator does not
finish the separate task of installing providers after Terminal Psi.

Boundary calls stay canonical in the checked program. Ordinary compilation
builds no receiver/requirement/entry-state dispatch associations, and neither
selection nor evaluation invents replacement source calls. Terminal
publication borrows the checked program directly. Package review reads the
same authored boundary calls and checks their write frames itself.

Settlement changes no checked body, so package source queries read the
settled trees directly. An operator application whose selected provider is a
checked adapter or a compiler-known float realization keeps naming the
operator requirement. Terminal Psi keeps that application, and receiving
admission joins its exact selected row before execution or native lowering;
the reference oracle's source-coordinate associations cannot replace that
join.

## Native realization

[Native-artifact emission](../backend/artifacts/native-artifact/src/lib.rs)
builds the final image and chooses between Direct and Dynamic ELF artifacts.
The coordinator supplies source-free views of the selected plans and passes on
the emitted object and retained evidence; the backend builds the complete
product and replays it independently. The compiler has no artifact assembler.

The [native compiler](../native-compiler/README.md) consumes Terminal Psi and
explicit realization inputs. Its public contracts are
[boundary realization](../../../../wiki/spec/terminal-psi/boundary_calls.md),
[private callbacks](../../../../wiki/spec/build/private_callbacks.md) and
[component publication](../../../../wiki/spec/build/component_publication.md).
Start at [compile.rs](../native-compiler/src/compile.rs): check scope
and entry, get the abstract input, admit providers, emit the object and
assemble the image. One `NativeCompileRequest` carries the image request,
optimization selections, admitted imports, component calls and authority
policies; the retained Terminal proposal supplies target and provider
evidence. None of these inputs selects another API entry point.

Program and callback realization pass admitted input to the shared
[native pipeline](../native-compiler/src/pipeline.rs), which runs abstract
optimization, target lowering and body emission. The
[native request](../native-compiler/src/request.rs) sets the work budget, and
the optimization stages decide how to spend it. The product and realization
owners admit callbacks and identities; the coordinator calls them in order.

[Object emission](../native-compiler/src/realization/object_emission.rs)
captures the target layout companions between lowering and emission, then
publishes the object with the callback bodies that survived. Callable objects
use the same steps; neither caller rebuilds the stage sequence. The target
stage keeps its validated translation, and the compiler adds no program copy.

The result tells direct and dynamic ELF artifacts apart; extracting a direct
one grants no authority to install dynamic output. Every rejection
returns the exact image request. Program-entry and callback-custody adapters
keep their extra owned evidence and use the same realization step.

Publication reads the abstract plan and its validated projection from the
evidence owner retained in the physical result, sharing that allocation
instead of carrying a second plan copy and separate identity fields.
Independent fragment, object and image replay still check the complete join.

Direct compilation and retained Terminal re-entry share
[source-binding preparation](../realization/provider-admission/src/source_evaluated_bindings.rs)
in the Terminal entry stage. It joins demanded imports and syscalls to exact
selected plans, derives import execution and stack inputs when none are
supplied, and rejects supplied lists that are incomplete or substituted. The
coordinator passes the prepared settlements on; it does not derive report
identities, assign foreign-call stack reserves or select import plans.
Provider admission derives each demanded leaf's mechanism and realization from
those settlements without rejoining their coverage, and the receiving
authority review classifies the result. Direct syscalls get execution
coordinates but no foreign same-stack reserve; their settlement checks the
exact kernel number, target and physical ABI. They still need general target
lowering and instruction emission: admission alone does not show that a
program using syscalls runs.

### Program-entry settlement

A native program must start with the entry contract that was reviewed for its
build. [Entry roots](../../../../wiki/spec/build/entry_roots.md) defines the
source and arrival contract. Before issuing its validated record, the
[shared entry settlement](../representations/abstract-operations/verified-optimization-input/src/entry_settlement.rs)
checks the target, source signature, calling and storage plans, artifact,
exact entry and service establishment itself; it does not ask the Psi receipt
producer to check its own output.

[entry_settlement.rs](../representations/abstract-operations/verified-optimization-input/src/entry_settlement.rs)
joins the selected Fused service evidence to the Terminal receiver's
attachment and exact erased fields, then to the selected provider plans. The
semantic receiver identity and the Terminal attachment identity are separate
facts and may be spelled differently. This join does not show that runtime
slots are published or that Independent execution works.

### Callback custody boundaries

Calling-policy execution takes a source-free `CallingPolicySignature`: the
`BoundarySignatureGeometry`, the target and all callback obligations. Its
input encoder and returned-plan validator sit together in
[policy_signature.rs](../realization/provider-planning/src/calling_policy_plans/policy_signature.rs),
and native application publication uses the same geometry. Source symbols
used by package review and layout preparation never reach the policy
activation. The catalog builder in
[terminal_signature.rs](../realization/provider-planning/src/calling_policy_plans/terminal_signature.rs)
produces the policy input directly and never builds a signature with
placeholder source symbols. Policy entries and final applications borrow one
Product publication owned by the invocation, fixed to the Psi optimization
chosen at source completion. Its roots are the authored entries, policy
bodies, constant providers and boundary declarations, whatever targets are
selected. There is no policy-only compilation. Named inspection keeps its own
explicit root through the same Psi operation without replacing Product code.
Preparation still reads source for ordinary and opaque layouts and for inline
callback slots; moving that work after Terminal publication is unfinished.
Preparation completes the native telescope (full parameter list) and every
callback destination before the single policy activation; provider schemas
record that final application, so no partial plan is later replaced.

Native boundary packing keeps no opaque source-selection record. It emits the
recipe's selection ordinal and native shape root, and provider planning joins
those occurrences to the original selection list once. Source symbols,
lifecycle and copy disposition stay with the review/publication owner, whose
checks are unchanged. Layout geometry alone cannot publish selected
representation custody.

Application entry, callback and adapter requests fix their source identities
before portable production. After sealing, Psi attaches their ordered root
judgments and Unit entry receipts using only those descriptions and the final
artifact. Module names for diagnostics are captured once; each application's
caller attribution follows the call graph from before optimization, not
reopened source or a path optimization removed. Shared executable roots come
from all authored Build bindings, not from whichever target's entry was
selected first.

Retained-product callbacks and checked-scope composition are done only when
native publication actually works, not when a zero-payload layout or
provider-selection check passes. Widening scalar-provider forwarding also
needs preserved incoming ABI homes, full call and relocation replay, and a
truly reachable authored entry. Test with rooted provider and callback
programs that keep full argument, result and resource custody; receipt
metadata tests alone do not finish the route.

A callback must keep the function and argument placement its calling contract
selected. Canonical callback applications and target calling applications stay
separate inputs to the realization request. The
[shared callback settlement](../representations/abstract-operations/verified-optimization-input/src/callback_settlement.rs)
joins their exact requirement and static binder ordinal, then produces the
[backend placement](../backend/plans/backend-plan/src/callback_artifacts.rs):
a direct callback parameter or record-field root with its native ordinal and
placement. Stored inputs keep canonical applications, not derived placements;
fresh and recovered products use the same placement step.
Source preparation keeps only the declaration checks and callback roots needed
to admit behavior before optimization, including unused registrar uses; ABI
and context replay and occurrence joins stay separate. The
[target-side carrier](../pipeline/02_abstract-operations-to-target-operations/src/coordination.rs)
binds a Terminal operation, placement index, private function, native
parameter application, registrar plan and context, and application commitment.
Target lowering checks the one-slot relation. The commitment only records its
producer: the reduced tuple cannot rebuild the authored parameter list or
prove which source site maps to which operation. Independent authentication
and replayable source correspondence are still needed before claiming full
publication custody; see the callback work on the
[execution board](../../../../TASKS.md).

The limited direct-parameter route supports one callback on the
normalized-import path, with fixed-integer arguments and results and full
register or stack placement. Field destinations and multiple callbacks need
more work. The checked callback body is a selected machine in the same
program, not a separately lowered artifact or identity namespace. Lowering
binds private code, native argument ordinal, relocation and final executable
region without inventing a Terminal operand.

The direct route supplies the declared native parameter, not a source-specific
host operation or compiler-inserted callback storage. Function and image
evidence do not establish registration or lifetime. Generic runtime
registration belongs to the authored host or runtime. Connecting a registrar
result to capacity, leases, source registration, cleanup and retry is
end-to-end runtime work, not a callback ledger in the compiler.
