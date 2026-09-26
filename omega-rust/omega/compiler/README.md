# Compiler coordination

[lib.rs](src/lib.rs) exposes source checking and one `CompileRequest`/`compile`
interface for any supplied target count. The coordinator sequences typed owner results; it does
not implement package loading, build evaluation, transformation algorithms or
visualization semantics. See the [pipeline map](../../pipeline.md).

[compiler.rs](src/compiler.rs) owns request admission, the shared checked
continuation, and the Check / Terminal / Native product dispatch. Its single public
function runs one target loop: checking, trust admission, observations, then the
requested product. There is no stateless wrapper object, finalizer callback, or
second native scheduler.
It prepares immutable source once, then drives each target's ordinary continuation.
[Native input reuse](src/native/native_product/input_reuse.rs) retains exactly matching
Terminal inputs as each target reaches realization. It does not retain every
child's checked trees until a second scheduling pass. Product reports belong to
their product owners; [package.rs](src/compiler/package.rs) owns package-custody
checks without calling back into the coordinator.

Checked-only consumers supply one `CheckedCompileRequest` to `compile_to_checked`.
Package inputs, build staging and session sponsors are request
data, not alternate compilation entrypoints. The request enters the same prepared
source continuation used by production and target batches.
Ordinary production accepts `CompileRequest::with_build_snapshot`, or distinct
`TargetCompileConfiguration::with_build_snapshot` requests. Each target captures
and executes its own caller-declared inventory and required-output roster;
parsing reuse does not merge those inputs or suppress a sibling's failure.
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
[scoped build inputs](../../../wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem).
The [checked entrance](src/checked/checking.rs) shows the lifecycle:
[admit and execute the build, then continue generated source](src/checked/checking/build_continuation.rs),
[check selected execution](src/checked/checking/execution_settlement.rs),
then [seal the result against current source custody](src/checked/checking/checked_compilation.rs).
The result retains the selected-execution settlement intact. Clones share program
storage until mutation; review instantiation borrow-projects table rows rather
than clone-rebuilding scratch. Any mutation does not reseal evidence, and
downstream reconstruction remains mandatory. `into_program` consumes the result and discards its compilation
evidence; compiler malformed-tree tests use that raw representation.

Candidate discovery can supply `CheckedCompileRequest::prepared_source_output`
to retain an opaque `PreparedCheckedSource`, then consume it with a fresh request.
The output slot is cleared before validation and published only on success.
This reuses physical source loading and parsing, never checked semantics or build
authority. Root paths and immutable package inputs must still match; callers retain
their before/after physical-source custody checks and final source-consumption
verification. Retention copies parsed storage for the discovery child and holds
that frontier until the final child consumes it; it is not a persistent cache.

[Native compilation](src/native/native_product/prepared.rs) prepares and realizes the native
product; it is not owned by optional optimization or report writing. Re-entry
from retained Terminal Psi uses `RetainedNativeRealizationRequest`. The current
API threads a receiving policy alongside the image request; separating its
mandatory admission gate from ordinary production is tracked by
`TWO-AXIS-TERMINAL-AUTHORITY-REVIEW`. The
[product contract](../../../wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission)
requires no receiving policy merely to emit an artifact. Explicit ecosystem
admission still checks its supplied policy. `CompileOutcomes` returns
one ordinary `CompileReport` or diagnostic result per target, without hiding failed
siblings. Single-product consumers explicitly use `into_single_report`, which
rejects any other cardinality. Executable publication remains a later operation.

## Product boundaries and observations

[Request validation](src/compiler/request.rs) checks the requested product and
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

[Checked admission](src/checked/admission/mod.rs)
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

Canonical source rows append directly into the retained production manifest;
report assembly does not build a second inventory of individually allocated
serialized rows. Source-consumption hashing reuses one row buffer. Both routes
use the package owner's canonical row encoder, preserving the existing byte
format and identities.

## Intrinsic settlement conversion

[intrinsic_settlements.rs](../build/provider-planning/src/compiler_intrinsics/mod.rs) validates all
selected plan provenance, then joins sorted borrowed intrinsic rows to lexical
Terminal demand. Original plan indices remain attached to each row. Duplicate
demanded rows reject; undemanded rows need no executable proposal. One exhaustive
optional mapping converts planner classifications into the native catalog and
rejects unsupported executions, as required by the
[boundary contract](../../../wiki/spec/terminal-psi/boundary_calls.md).

## Target set and realization

[Explicit targets](src/compiler/request/targets.rs) accepts an optional
selection: absent, the realization set is every deployable profile the
toolchain closure provides; present, a nonempty set normalized to canonical
order with duplicates removed, with wildcards, empty and unknown selections
rejected. The selection narrows realization only. Psi checks every target's
bodies once per compilation regardless
([multi-target compilation](../../../wiki/spec/build/configuration.md#multi-target-compilation)).

[Request admission](src/compiler/request.rs) stores root, product, observation
policy, package sources and the target set once. Compilation collects one
ordered outcome per realized target without fail-fast; a shared checking
failure supplies the same diagnostics to every realization, and one
realization's failure never suppresses another's. Success retains the ordinary
standalone artifact/manifest identity. The collection carries an optional
batch manifest binding the realization set and each realization's
commitment/outcome; it grants no support, test or audit claim.

Today's implementation instead retains one immutable source parse checkpoint
([source preparation](../build/build-evaluation/src/sources/source_assembly/checkpoint.rs))
and runs an exact-target child per configuration: per-target source assembly,
declaration filtering, build evaluation, provider settlement and checking, with
request-level policy copied into each configuration.
[Generated-source continuation](../../psi/pipeline/03_symbol-resolved-trees-to-typed-trees/README.md)
owns append custody and [checked settlement](#checked-settlement) owns its
later ordered joins. The
[pipeline route items](../../../TASKS.md#pipeline-route) replace the child
route with the single pass above; until then
[prepared-input reuse](#prepared-input-reuse)
shares prepared inputs across children by exact artifact/profile/selection key,
and shared parsing never permits reusing one child's checked target, providers,
admissions or physical evidence as another child's authority.

## Checked settlement

Psi owns [source checking](../../../wiki/spec/language/state_contracts.md).
This coordinator closes build/target inputs around that phase; it does not add
target policy to the portable Terminal module. Enter
[phase_transitions.rs](src/checked/checking/phase_transitions.rs).

`CheckedProgramSurface` retains checked Psi with exact predecessor facts:
selected provider plans/grants, callback placements, accepted template
classifications, and contract-entailment stand-downs. `TypedToCheckedSettlementInput`
closes callback and provider receipts transactionally before checked ownership
is shared. A compact identifier is not a replacement for these retained inputs.

Selected execution then produces `SelectedExecutionSettlementSurface`. Its
ordered work closes component-entry progress, selected-dispatch settlement,
provenance, boundary-adapter associations, and task activations. Preserve that order and
each independent sidecar when moving coordinator code; duplicating or reordering
settlement is not a new pipeline stage.

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

This coordinator consumes canonical Terminal Psi and explicit realization
inputs. Its public contracts are [boundary realization](../../../wiki/spec/terminal-psi/boundary_calls.md),
[private callbacks](../../../wiki/spec/build/private_callbacks.md), and
[component publication](../../../wiki/spec/build/component_publication.md).
Start at [native_realization.rs](src/native/native_realization.rs):
validate scope and entry, obtain the abstract input, admit providers, emit the
object, and assemble the image. One `NativeRealizationRequest` carries the image
request, optional checked scope and optional prepared input alongside target and
provider evidence. Those inputs do not select alternate API entrypoints.

The result distinguishes direct and dynamic ELF artifacts; extracting a direct
artifact cannot grant dynamic output installation authority. Every rejection
returns the exact image request. Program-entry and callback-custody adapters
retain their additional owned evidence and use the same realization operation.

Publication reads the abstract plan and its validated projection from the
physical result's retained evidence owner. It shares that allocation instead
of carrying a second plan copy and separately supplied identity fields.
Independent fragment, object, and image replay still check the complete join.

### Prepared-input reuse

Until Psi runs once per compilation, each target prepares its own Terminal
artifact. [PreparedNativeRealizationInput](src/native/native_realization/input_preparation.rs)
shares target-neutral decoding, proof admission and abstract-input lowering
only for equal complete `TerminalArtifactIdentity`, exact `AdmissionProfile`
and exact `PostTerminalOptimizationSelections`, and rechecks that key on use.
Target, entry and calling plans, provider and external settlements, authority
policies, callbacks, FMA admission and physical evidence stay per target. The
reuse is an optimization, not review, proof or audit evidence.

### Program-entry settlement

[Entry roots](../../../wiki/spec/build/entry_roots.md) owns the source/arrival
contract. [entry_settlement](src/native/entry_settlement/mod.rs) independently replays
target, source signature, calling/storage plans, canonical artifact, exact
Terminal entry, and service establishment before issuing the validated carrier.
It does not call the Psi receipt producer to validate that producer's output.

[service_establishment.rs](src/native/entry_settlement/service_establishment.rs) joins
selected Fused service evidence to the Terminal receiver's attachment and exact
erased fields, then to the selected provider plans. Semantic receiver identity
and retained Terminal attachment identity are separate facts and need not have
the same spelling. The [entry controls](src/native/tests/native_realization/entry_settlement.rs)
cover this join; it is not evidence of runtime slot publication or Independent
execution support.

### Callback custody boundaries

Retained-product callback and checked-scope composition must close through
actual native publication, not a zero-payload layout or provider-selection check.
Widening scalar-provider forwarding additionally requires preserved incoming
ABI homes, complete call/relocation replay, and a genuinely reachable authored
entry. Use rooted provider and callback controls retaining complete argument,
result, and resource custody; metadata-only receipt tests do not close the route.

[callback_custody.rs](src/native/native_realization/callback_custody.rs) returns the caller's
opaque companion by value on both success and rejection. That wrapper does not
admit, lower, fingerprint, or interpret its contents.

Native callback arguments separately enter the realization request. Their
[target-side carrier](../pipeline/02_abstract-operations-to-target-operations/src/lowering/coordination/native_callbacks.rs)
binds a Terminal operation, placement index, private function, native parameter
application, registrar plan/context, and application commitment. Target lowering
validates the one-slot relation. The commitment remains producer provenance:
the reduced tuple cannot reconstruct the complete authored telescope or prove
the source-site-to-operation mapping. Independent authentication and replayable
source correspondence remain required before claiming complete publication
custody; see the callback work on the [execution board](../../../TASKS.md).

The bounded direct-parameter route retains one callback on the normalized-import
path with fixed-integer semantic arguments/results and complete register or
stack placement. Field destinations and multiple callbacks need further work.
The checked callback body is a separate canonical artifact, with its own local
machine namespace. Lowering must bind private code, native argument ordinal,
relocation, and final executable region without inventing a Terminal operand.

The direct route supplies the declared native parameter, not a source-specific
host operation or compiler-inserted callback storage object. Function/image
evidence does not establish registration or lifetime. Generic runtime registration
primitives in [component-publication](../backend/runtime/component-publication/src/callback_registration.rs)
are separate from connecting an authored registrar result to capacity, leases,
source registration, cleanup, and retry. Their existence alone does not close
that end-to-end path.
