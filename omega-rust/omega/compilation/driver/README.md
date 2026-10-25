# Compiler coordination

[lib.rs](src/lib.rs) exposes source checking and one `CompileRequest`/`compile`
interface for any supplied target count. The coordinator sequences typed owner results; it does
not implement package loading, build evaluation, transformation algorithms or
visualization semantics. See the [pipeline map](../../../pipeline.md).

[Terminal production](terminal-production/README.md) is the shared stage-05–07
sequence used by ordinary and package compilation. It is a narrow coordinator
package under this driver, not a Psi transform. Permission-ledger validation
belongs to stage 05; entry eligibility, source receipt joins and publication
belong to stage 07. Their language rules do not live in the coordinator.

The independent [native compiler](../native-production/README.md) owns the
Terminal-to-image sequence behind a source-free dependency boundary. This
coordinator calls it after Terminal production; it no longer owns native stages.
Consumers use
`native-artifact`, `image-emission`, and stage 00's `provider_admission` directly
for those contracts. Reports and publication come from `compilation-products`;
there is no compiler-owned `report` or `artifact_publication` module. Root-level
request/result exports remain conveniences for the compilation entrypoints.

[compiler.rs](src/compiler.rs) sequences request admission, one source/Build
prefix, then the Check / Terminal / Native product dispatch. It settles all
targets before producing Terminal. Targets with the same checked source owner,
and Psi optimization share that production and artifact storage; target
receipts and exclusion judgments stay separate. Native production receives the
artifact explicitly and never calls Psi. Distinct roots and callbacks belong
to the shared whole-program Terminal entry roster.
`cargo run -p compiler --example terminal_sharing` exercises two native targets
through this route and checks actual production counts and shared backing
storage. It does not execute a foreign-host image.
`--check` and `--terminal` exercise the other report stops. For target-local
failure isolation, use `--reject linux_arm64
tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg
windows_x86_64 linux_arm64 macos_arm64`: the missing entry rejects without
discarding either successful sibling.

The coordinator retains an ordered roster of completed outputs, errors and
pending Terminal work. It borrows only the pending work for shared production,
then finishes each application in that same order; no indexed output holes or
parallel pending/result tables are reconstructed. [Product execution](src/product_execution.rs)
owns one application's stop, permission admission, installation and output.
[Product report planning](../products/src/checked_compilation/report.rs) owns
the requested output together with native production inputs, and settles the
optimization rollback once. The same choices feed Terminal, native production
and the final receipt. A native request is carried by its native report
settlement, not compared against an independent requested-mode tag afterward.

The [session](src/checked/compilation_session.rs) owns the shared source/Build
continuation and retained checking. Provider selection still precedes Terminal:
the producer needs selected entry/callback roots and exclusion inputs.
Installation, permission comparison and trust admission keep their existing
ordering after portable production. This sequencing cleanup does not establish
the separate, unfinished Terminal-only provider handoff.
The independent [product request owner](../products/src/compile_request.rs)
expands explicit targets over one invocation policy, rejects duplicates and incompatible rollback,
and admits evaluated Build requirements. It returns immutable per-target inputs;
the coordinator owns the loop, not those policy decisions.
It assembles, resolves, executes Build and completes authored/generated typing
once. [Prepared source products](../products/src/prepared_build.rs) carry that
checked prefix and sealed source custody, not a cache keyed by target inputs.
Provider settlement borrows those owners and cannot mutate checked metadata.
Package review enters the same operations with its own admitted activation.
Its compiler session shares checked storage, and package targets finish through
the common Terminal batch. Source completion still needs convergence with the
whole-program request. Native production consumes each
target's retained Terminal artifact without a second scheduling pass. Product
reports belong to their product owners.

Accepted package compilation supplies dependency-ordered
`PackageCompilationRequest`s together to the session's
[`compile_packages`](src/checked/package_compilation.rs) entrance, with the
requested product and root permission consent supplied up front. That operation
advances dependency-first [package frontiers](src/checked/package_preparation.rs)
across the requested products. Each round joins generated dependency outputs,
executes the next admitted Builds and retains their pending Product continuations.
Selected independent components enter source completion together before their
consumers advance. Package code supplies custody, consent and staging, not
per-Build compiler calls or continuation tables. Independent-component publication
belongs to [package.rs](src/compiler/package.rs), including when review requests
a description. [Product batching](src/checked/product_batch.rs) partitions the
admitted root and component continuations by complete source input and checks each partition
once. Matching members borrow its checked program and sealed source custody;
each retains its own Build receipts, charges, timings and target settlement.
This is work partitioning within the submitted batch, not a historical cache.
Opaque pending evaluation authorities must be the same retained authority;
matching paths or settings alone never establish interchangeable continuations.
`mbx run -p package-manager --example package_batch` asserts whole-invocation
work through the public package route. Counters start before local-project
preparation and remain checked when production rejects. Its default two-target
fixture requires one Build execution, two settled checks (Build and Product),
one source completion and one shared Terminal allocation. `--check` preserves
the execution/check counts and produces no Terminal program. Each occurrence
still admits its own Build effects and charges; only exact matching Pure Builds
reuse computation. Components demanded in different dependency rounds do not
share completion through this batch.

The ordinary dependency control retains two different Builds and one Product
check rather than merging their work:

```sh
mbx run -p package-manager --example package_batch -- tests/omega/pass/build/package_batch_distinct_builds/main.omg 1 1 2 3
mbx run -p package-manager --example package_batch -- --check --targets linux_x86_64 tests/omega/fail/build/unrequested_target_entry_returns_value/main.omg 1 0 1 2
```

The latter must reject `foreign_launch` because `ProgramEntry` has no result,
even though its Windows row is unrequested. Expected counts are source-derived
acceptance inputs, never filled from the observed counters. Supplying two
expected Builds to the default equal-source fixture must fail its assertion.
Counts are process-wide: run each measurement alone.

`mbx run -p compiler --example package_batch_sources` submits three products
from two real source roots to the compiler's public package facade. Equal
consumers share one checked/Terminal cohort; the different root retains another.
The batch requires two Builds, four checks, two source completions and two
Terminal productions, with shared storage only for the equal-source pair.
This control uses the documented repository fixture acceptance, not a production
package audit. Its fixture preparation legitimately performs another two Builds
and four checks in its own sessions, so the whole measurement asserts four
Builds/eight checks rather than hiding setup cost. These workflow measurements
advance `BUILD-EVALUATES-ONCE`; they do not close its remaining target-row work.
The package operation owns one compiler-stack worker; preparing Builds and
checking source cohorts do not spawn additional workers.

Package operations receive reports or Terminal inspections, never a checked
checkpoint to return for production. The compiler compares fresh supplied
permissions with the exact root Product consent before native production,
using the independent package-evidence projection owner. Inspection is an
explicit product with no native authority. Standalone inspection and corpus
production submit the same `CompilationProduct` through `CompilationSession::produce`;
source checking returns selected provider custody internally. Standalone and
package products both produce Terminal before installation; there is no
already-installed checked-to-product continuation. Check-only and artifact-only
requests stop without Terminal production. Review descriptions borrow the same
Terminal production inputs without making the producer branch on review history.

Checked-only consumers supply `CheckedCompileRequest` to `CompilationSession::check`.
Package inputs, build staging and session sponsors are request
data, not alternate compilation entrypoints. The request enters the same prepared
source continuation used by production and target batches.
Ordinary production accepts one `CompileRequest::with_build_snapshot` inventory
and required-output roster. `CompileRequest::for_targets` takes the package
root, one Build staging directory and the selected profiles. Artifact publication
separately owns output placement. Targetless Check uses one absent profile.
Different caller inventories are different
invocations, not target variants of one compilation. Each target independently
admits the supplied custody without suppressing a sibling's failure.
Package [compilation reuse](../../packages/compilation-inputs/src/compilation_reuse.rs)
owns the source comparison: assembly observes imported content and entry
bindings. There is no per-target Build result cache or live-output reuse key.
Build observes its invocation's single staging account; its captured outputs
may be published alongside several target products without executing it again.
Source-reuse keys never confer Build authority.
An omitted request retains automatic capture of canonical package inputs.
Standalone scoped inventories must include consumed source files and do not
expose undeclared siblings. Without a caller-supplied sponsor, captured builds
use fresh private output staging, released after retained bytes are captured.
Artifact-only builds return completed files without requiring a program entry;
executable builds retain the same file carrier for companions. Publication writes
only completed files under `completed/<set-identity>/files/`, with `manifest.bin`
binding the exact build observation, logical paths, lengths and byte digests.
Existing sets are checked against retained bytes, not trusted by directory name.
Scratch never publishes, and a failed compilation returns no successful set. See
[scoped build inputs](../../../../wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem).
The [checked entrance](src/checked/checking.rs) shows the lifecycle:
[evaluate Build and continue generated source](src/checked/checking/build_continuation.rs),
[check Product and seal source custody](src/checked/checking/source_checking.rs),
then [select and install each target](src/execution_settlement.rs).
The Build coordinator calls the [Psi source continuation](../../../psi/compilation/driver/src/source_compilation.rs)
for resolution, typing and generated-source completion. It owns package admission
and Build authority, not the resolver/typing fixed points or their paired arenas.
[Psi source checking](../../../psi/compilation/driver/src/source_checking.rs) then
owns preparation, retained-judgment matching, checking and product pruning.
Build admission uses that same checking sequence. Shared source projections and
operator queries belong to semantic checking libraries, not a stage backdoor.
The product owner retains the complete checked result, its evaluated Build and
source custody, and owns metadata and production-subject projections. Its shared
Build records are data contracts consumed by evaluation and product assembly;
they do not import the evaluator or coordinator. Compilation still visibly
schedules the stages and supplies their complete results to sealing.
The continuation calls resolution and typing directly, including generated-source
extension, with timing around those actual calls rather than another transition
module. The existing [Build evaluator](../build-evaluation/src/build_evaluation.rs)
owns the complete pre-effect admission/consent/execution protocol and returns
configuration, restricted requests and the admitted source commitment together.
Its private declaration-admission operation still uses the preliminary Psi
checked pass; eliminating that duplicate semantic work is separate from moving
the protocol out of the coordinator. There is no compiler-private package gate
or public raw Build checkpoint that can bypass the evaluation protocol.
Checked-product selection belongs to
[Psi stage 04](../../../psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/product_pruning/mod.rs):
the coordinator supplies the requested roots after rollback; the stage owns the
identity case, root validation, pruning and selection evidence. It consumes only
checked trees, not the Omega provider-settlement carrier.
The result retains the selected-execution settlement intact. Its evaluated Build
is one shared immutable product: configuration, observations, usage, optimization
report intent and generated outputs are not dismantled into parallel fields.
Generated-source bundles project the same retained outputs; source custody only
adds their source-coordinate join. Build consent and the selected-machine check
run visibly before and after execution, without extra checkpoint couriers.
Restricted host-request vocabulary and normalization belong to package
[build-execution custody](../../packages/compilation-inputs/src/build_execution/restricted_requests.rs),
beside the filesystem grants, captured inventories and sponsor limits they
describe. Build evaluation checks consent under its admitted execution mode
before execution. Review and encoding consume that package
contract directly, without reconstructing it from compiler-owned records.
Authored Build policy remains distinct from per-target realized evidence, and
checked-product equality includes the complete evaluated Build. Clones share program
storage until mutation; review instantiation borrow-projects table rows rather
than clone-rebuilding scratch. Any mutation does not reseal evidence, and
downstream reconstruction remains mandatory. `into_program` consumes the result and discards its compilation
evidence.

Candidate discovery calls `CompilationSession::check(request)` for the verdict
and independent-component discovery. The session owns source frontiers, Pure
Build values, exact language judgments and Product root selections; callers
carry none of those caches. Root paths and immutable package inputs must match,
and every activation admits current custody, grants, selections and sponsor
charges. Evaluated selections survive a later settlement rejection so review
can supply component descriptions and account for the rejected attempt.
Successful language judgments share checked storage, not authority. All reuse
is invocation-local. Package targets finish together through `finish_compilation`
so equal checked programs produce Terminal Psi once before native realization.

[Native product preparation](src/native.rs) produces the same retained
Terminal product as Terminal-only compilation, then consumes it through
[`native_production::compile`](../native-production/src/native_compilation.rs).
Fresh compilation and package re-entry share one source-free realization route:
there is no direct-source native driver, second program-entry artifact, or
prepared-compilation cache lifecycle. Package acceptance is captured separately
from the caller's optional receiving policy; neither substitutes for the other. The
[product contract](../../../../wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission)
requires no receiving policy merely to emit an artifact. Explicit ecosystem
admission still checks its supplied policy. `CompileOutcomes` returns
one ordinary `CompileReport` or diagnostic result per target, without hiding failed
siblings. Single-product consumers explicitly use `into_single_report`, which
rejects any other cardinality. Executable publication remains a later operation.

[Compilation products](../products/README.md) owns that operation:
`publish_compilation(report, directory)` selects the retained Native, Terminal,
or build-only product, installs its requested proof pairs, then publishes
completed build companions. The CLI and library callers use this same entrypoint.
Its `CompileReport` owns payload cardinality and receipt joins, not filesystem
orchestration. Neither those domain decisions nor publication live in this driver.
Check-only and already-published reports reject before filesystem mutation.
Backend artifact owners still construct native evidence; platform
custody owns staging and installation mechanics.

## Supporting packages

This directory contains only compilation coordination. Build activation belongs to
[realization/build-evaluation](../build-evaluation/README.md), an
independent package consumed by the coordinator and package review.
Project declaration grammar, retained build outputs and compilation-input custody
belong to the package system's independent
[declarations](../../packages/build-declarations/README.md),
[outputs](../../packages/build-output/README.md) and
[inputs](../../packages/compilation-inputs/README.md) packages. Consumers
depend on those contracts directly, not on the package-command application.

Target-neutral build evaluation remains in
[Psi's compiler support](../../../psi/compilation/compile-time/README.md).
Opaque representation admission belongs to
[Psi checking](../../../psi/compilation/checking/representation-planning/README.md);
provider selection and planning belong to
[Omega provider realization](../realization/provider-planning/README.md), beside
[selected execution](../realization/selected-dispatch/README.md).
Trust obligation and report reconstruction belong to
[realization admission](../realization/trust-model/README.md).
Host output writing and opt-in measurements belong to the
[artifact utilities](../../foundation/artifacts/README.md); these utilities retain
no semantic trust records.
Package installation and update belong to the separate
[package manager](../../packages/package-manager/README.md).

## Product boundaries and observations

The [Terminal coordinator](src/terminal/terminal_artifact.rs) admits composition
and exact-entry selection, then asks the Psi facade for one shared program and
per-application entry receipts. Psi runs authored exclusion admission before
optimization. Provider installation and native-proposal construction follow
production; neither restarts Psi or independently verifies its fresh output.
Callback placements stay in Omega rather than travelling through
Psi as an opaque payload. One effective Psi optimization selection governs the
whole program, including callback roots. The coordinator joins callback entries
in that shared artifact before asking
[Omega stage 00](../pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal/proposal.rs)
to construct the native proposal. Canonical semantic decoding, selected-provider
demands, entry/comparison/boundary joins and callback placement replay belong to
that stage, not to the compiler. Missing callback bodies reject during shared
production or its exact declaration join; neither step executes an artifact.

Terminal production borrows the unoptimized program for authored exclusions,
checking each application's main and selected callback roots before optimization.
Exclusion admission uses the backend effects checker; on rejection,
[Psi lowering's source attribution](../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/lowered_machine_provenance.rs)
supplies machine names and declaration spans. That diagnostic mapping is not
admission evidence.

[Request validation](../products/src/compile_request.rs) checks the requested product and
rollback selection before acquiring source or opening output directories.
Checking permits no rollback; Terminal publication permits only applicable
Psi-stage rollback; native products permit their applicable phase set. Rollback
is subtractive, not a new optimizer selection or a fallback backend.

A retained native-artifact request runs the checked frontend and canonical
Terminal producer, then the same source-free
[native realization](#native-realization) used for component staging.
The non-clonable result retains Terminal identity, target, selected providers,
object/relocation evidence, encoded text and independently replayed image.
It writes no primary output and grants no executable path, installation,
publication, deployment or runtime authority. Pending component progress cannot
be silently discarded. Unsupported Terminal vocabulary rejects without another
source-shaped path. [Native artifacts](../backend/artifacts/native-artifact/README.md)
owns the current physical evidence limits.

Compilation produces the requested product and diagnostics. It does not construct
or write optional JSON, HTML, text or Markdown debug dumps, disassembly reports,
or timing files. There is no full/output-only observation policy.

[Trust admission](../realization/trust-model/README.md)
still reconstructs trust obligations, settles exact owner admissions and validates
target/report consistency. Wire compatibility, capability validation, package
locks and required product evidence remain mandatory. A check or retained-artifact
compile need not create a build directory; explicit build effects and primary
publication have their own filesystem requirements.

The CLI's `--timings` option prints preparation, compilation and native publication
durations (where applicable), followed by total elapsed time, on stderr. These are
command stages, not an exhaustive internal pass profile. A failed command still
reports its elapsed total. Normal compilation leaves timing collection disabled
and does not install an allocation-counting global allocator.

Source-consumption hashing reuses one row buffer and the package owner's
canonical row encoder, preserving the existing byte format and identities.

## Intrinsic settlement conversion

[Native builtin projection](../pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/compiler_builtins/proposals.rs) validates all
selected plan provenance, then joins sorted borrowed intrinsic rows to lexical
Terminal demand. Original plan indices remain attached to each row. Duplicate
demanded rows reject; undemanded rows need no executable proposal. One exhaustive
optional mapping converts planner classifications into the native catalog and
rejects unsupported executions, as required by the
[boundary contract](../../../../wiki/spec/terminal-psi/boundary_calls.md).
It consumes source-free plan and execution-identity views; provider planning
does not depend on target-operation IR. Native admission independently rejoins
the proposals to the receiving target catalog.

## Target set and realization

[Explicit targets](../backend/target/src/explicit_target_set.rs) accepts an optional
selection: absent, the realization set is every deployable profile the
toolchain closure provides; present, a nonempty set normalized to canonical
order with duplicates removed, with wildcards, empty and unknown selections
rejected. The selection narrows realization only. Psi checks every target's
bodies once per compilation regardless
([multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation)).

[Request admission](../products/src/compile_request.rs) stores root, product, observation
policy, package sources and the target set once. Compilation collects one
ordered outcome per realized target without fail-fast; a shared checking
failure supplies the same diagnostics to every realization, and one
realization's failure never suppresses another's. Success retains the ordinary
standalone artifact identity. The collection grants no support, test or audit
claim.

Today's implementation instead retains one immutable source parse checkpoint
([package source preparation](../../packages/compilation-inputs/src/source_assembly.rs))
and runs an exact-target child per configuration: per-target source assembly,
declaration filtering, build evaluation, provider settlement and checking, with
request-level policy copied into each configuration.
[Generated-source continuation](../../../psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/README.md)
owns append custody and [checked settlement](#checked-settlement) owns its
later ordered joins. The
[pipeline route items](../../../../TASKS.md#pipeline-route) replace the child
route with the single pass above; until then each child prepares its own
Terminal input, and shared parsing never permits reusing one child's checked
target, providers, admissions or physical evidence as another child's authority.

## Checked settlement

Physical, dependency-generated and activation-generated parsing all belong to
[package source preparation](../../packages/compilation-inputs/src/source_assembly.rs).
The coordinator forwards its complete syntax products into resolution and
typing. Loading, lexing, parsing and syntax construction are not compiler-private
continuation work; parser dependencies are needed only by corpus fixtures.

Psi owns [source checking](../../../../wiki/spec/language/state_contracts.md).
Its checking stage also owns exact boundary-call and source-authority validation;
the compiler does not issue a separate source-provider approval after checking.
It forwards independently rederived opaque-representation property receipts;
Psi rejoins their authored dependency-scope instances during input preparation.
The coordinator supplies evaluated build authority to Psi's checking request;
Psi checks assembly authority even when the language judgment is memoized.
Language-only probes do not issue runtime installation authority.

[Source checking](src/checked/checking/source_checking.rs) consumes Product typing
once, admits representation properties and seals source custody before target
settlement. Targets share both the checked arenas and sealed custody.
[Provider binding](../realization/selected-dispatch/src/provider_binding.rs)
belongs to Omega realization. It binds callback placements and provider grants
without preparing or rewriting Psi's checked result. Psi
derives candidate specializations from authored conformances independently of
runtime provider selection. `CheckedProviderProgram` retains the resulting program
and exact review records; the coordinator does not reconstruct them.

Provider-dependent Product constants currently reject with an explicit
`unimplemented:` diagnostic. Portable symbolic recipes must cross Psi before
realization closes them; selected runtime values cannot be folded into shared
source. Admitted Build evaluation retains its separate evaluation authority.
Selected execution preparation belongs to Psi: one Omega authority pass joins
each source requirement to an exact intrinsic or checked adapter and plan
commitment. Psi derives the Float operation and carriers or provider entry,
then independently validates source selection, operands, arithmetic policy,
signature, conformance, and invocation admission. The coordinator does not
classify Float meaning or rebuild separate fact lists for these two supplies.

[Execution settlement](../realization/selected-dispatch/src/execution_settlement.rs)
produces `SelectedExecution`: component-entry progress, dispatch associations,
accepted-service provenance and task activations. It retains Psi's checked
execution plans instead of re-entering Psi to rebuild them. This is still a
checked-tree realization seam; moving it out of the coordinator does not
complete the separate task of installing providers behind Terminal Psi.

Boundary calls remain canonical in checked trees. Selected execution records
exact receiver/requirement/entry-state associations and whether the receiver is
forwarded. The interpreter consumes those associations before host dispatch;
neither selection nor evaluation fabricates replacement source calls. Terminal
publication borrows the checked program directly. Package review reads the same
authored boundary calls and independently checks their write frames.

Settlement changes no checked body, so package source queries read the
settled trees directly. An operator application whose selected provider is a
checked adapter or a compiler-known float realization keeps naming the
operator; until Terminal Psi carries a requirement-level operator application
for Omega to install, the machine applying it has no Unit plan and the
omission reports `unimplemented:`.

## Native realization

Final image construction and Direct/Dynamic-ELF artifact dispatch belong to
[native-artifact emission](../backend/artifacts/native-artifact/src/native_artifact/emission.rs).
The coordinator supplies source-free selected-plan projections and forwards the
emitted object and retained evidence; the backend constructs and independently
replays the complete product. No compiler-local artifact assembler remains.

The [native compiler](../native-production/README.md) consumes canonical Terminal Psi and explicit realization
inputs. Its public contracts are [boundary realization](../../../../wiki/spec/terminal-psi/boundary_calls.md),
[private callbacks](../../../../wiki/spec/build/private_callbacks.md), and
[component publication](../../../../wiki/spec/build/component_publication.md).
Start at [native_compilation.rs](../native-production/src/native_compilation.rs):
validate scope and entry, obtain the abstract input, admit providers, emit the
object, and assemble the image. One `NativeCompileRequest` carries the image
request, optimization selections, admitted imports and authority policies.
The retained Terminal proposal supplies target and provider evidence. Those
inputs do not select alternate API entrypoints.

Program and callback realization pass verified abstract input directly to
[Omega's abstract optimization stage](../pipeline/01_abstract-operations-to-abstract-operations/src/abstract_optimization.rs),
then feed its validated plan into target lowering. The
[native request](../native-production/src/request.rs)
supplies the work budget; optimization stages own its use. Callback and identity
admission belong to the product and realization owners. The coordinator sequences
those calls.

[Object emission](../native-production/src/realization/object_emission.rs) feeds the
target-lowering result directly into the shared physical sequence. That sequence
returns the backend's fragment-emission input, which publication, callback
emission and optimization reporting consume directly. No compiler-owned program
copy or result wrapper sits between these owners; the target stage retains its
validated translation, and fragment emission independently replays physical custody.

The result distinguishes direct and dynamic ELF artifacts; extracting a direct
artifact cannot grant dynamic output installation authority. Every rejection
returns the exact image request. Program-entry and callback-custody adapters
retain their additional owned evidence and use the same realization operation.

Publication reads the abstract plan and its validated projection from the
physical result's retained evidence owner. It shares that allocation instead
of carrying a second plan copy and separately supplied identity fields.
Independent fragment, object, and image replay still check the complete join.

Direct compilation and retained Terminal re-entry share the Terminal entry stage's
[source-binding preparation](../pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/source_evaluated_bindings.rs).
It joins demanded imports and syscalls to exact selected plans, derives import execution and stack
inputs when none are supplied, and rejects incomplete or substituted supplied
rosters. The coordinator forwards the prepared settlements; it does not derive
report identities, assign foreign-call stack reserves, or select import plans.
Provider admission derives each demanded leaf's mechanism and realization from
those settlements without rejoining their coverage, and the receiving authority
review classifies the result.
Direct syscalls derive execution coordinates but no foreign same-stack reserve;
their settlement checks the exact kernel number, target and physical ABI.
They still require general target lowering and instruction emission. Admission
alone is not evidence that a syscall-backed program runs.

### Program-entry settlement

[Entry roots](../../../../wiki/spec/build/entry_roots.md) owns the source/arrival
contract. [entry_settlement](../pipeline/00_terminal-psi-to-abstract-operations/src/entry_settlement/mod.rs) independently replays
target, source signature, calling/storage plans, canonical artifact, exact
Terminal entry, and service establishment before issuing the validated carrier.
It does not call the Psi receipt producer to validate that producer's output.

[service_establishment.rs](../pipeline/00_terminal-psi-to-abstract-operations/src/entry_settlement/service_establishment.rs) joins
selected Fused service evidence to the Terminal receiver's attachment and exact
erased fields, then to the selected provider plans. Semantic receiver identity
and retained Terminal attachment identity are separate facts and need not have
the same spelling. This join is not evidence of runtime slot publication or
Independent execution support.

### Callback custody boundaries

Retained-product callback and checked-scope composition must close through
actual native publication, not a zero-payload layout or provider-selection check.
Widening scalar-provider forwarding additionally requires preserved incoming
ABI homes, complete call/relocation replay, and a genuinely reachable authored
entry. Use rooted provider and callback controls retaining complete argument,
result, and resource custody; metadata-only receipt tests do not close the route.

Native callback arguments separately enter the realization request. Their
[backend placement](../backend/plans/backend-plan/src/callback_placements.rs)
owns destination application selection and consistency: a direct callback
parameter or a record-field root, with its exact native ordinal and placement.
Proposal production, retained-product replay and native admission consume that
same checked projection; ABI/context replay and occurrence joins remain separate.
The
[target-side carrier](../pipeline/02_abstract-operations-to-target-operations/src/lowering/coordination/native_callbacks.rs)
binds a Terminal operation, placement index, private function, native parameter
application, registrar plan/context, and application commitment. Target lowering
validates the one-slot relation. The commitment remains producer provenance:
the reduced tuple cannot reconstruct the complete authored telescope or prove
the source-site-to-operation mapping. Independent authentication and replayable
source correspondence remain required before claiming complete publication
custody; see the callback work on the [execution board](../../../../TASKS.md).

The bounded direct-parameter route retains one callback on the normalized-import
path with fixed-integer semantic arguments/results and complete register or
stack placement. Field destinations and multiple callbacks need further work.
The checked callback body is a selected machine in the same canonical program,
not a separately lowered artifact or identity namespace. Lowering binds private code, native argument ordinal,
relocation, and final executable region without inventing a Terminal operand.

The direct route supplies the declared native parameter, not a source-specific
host operation or compiler-inserted callback storage object. Function/image
evidence does not establish registration or lifetime. Generic runtime registration
belongs to the authored host/runtime; connecting a registrar result to capacity,
leases, source registration, cleanup and retry remains end-to-end runtime work,
not a compiler-side callback ledger.
