# Terminal Psi operation vocabulary

> **Needs porting.** This document has not been consolidated or vetted for the
> current documentation structure. See the [migration index](../../README.md).

[Documentation index](../../../README.md) | [Pipeline](pipeline.md)

The [portable-product contract](../../../spec/terminal-psi/product.md),
[boundary-call and realization contract](../../../spec/terminal-psi/boundary_calls.md),
the [immutable byte-view vocabulary](../../../spec/terminal-psi/byte_views.md), and
[observations](../../../spec/terminal-psi/observations.md),
[encoding](../../../spec/terminal-psi/encoding.md),
[verification](../../../spec/terminal-psi/verification.md),
[calls and outcomes](../../../spec/terminal-psi/calls_and_outcomes.md),
[mathematical proof values](../../../spec/terminal-psi/mathematical_values.md),
[integer certificates](../../../spec/terminal-psi/integer_certificates.md), and
[logical work](../../../spec/resources/logical_work.md) have moved to their
specification owners. This remaining reference carries operation-specific
meaning and validation details until their consolidation;
it does not redefine those migrated subjects.

Implementation entry maps live beside
[Terminal production](../../../../omega-rust/psi/compiler/terminal-production/README.md)
and [Terminal-to-abstract lowering](../../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md).
The [execution board](../../../../TASKS.md) owns unfinished implementation.
Current pre-release producers and consumers move together; stale artifacts reject.

## Terminal requirements

Terminal Psi is immutable and self-contained. It contains no arena handle that
requires `TypedTrees`, source syntax, the producer compiler, or instruction
selection to interpret its meaning. It contains:

- concrete machines and instantiated types;
- explicit typed blocks, block parameters, values, calls, transitions,
  continuations, and terminals;
- lowered predicates over the same stable value/place identities as execution;
- typed structural places, including ordinary and provider-backed roots plus
  field, dynamic-index, dereference, and range/subextent projection;
- explicit cleanup, transfer, conservation, invalidation, and boundary actions
  on edges, plus suspension plans on the exact incomplete calls they govern;
- closed semantic operation variants, including scoped CPU/device ordering
  events; and
- fingerprinted contracts, obligation schemas, authorized admission sites,
  trust attribution, and work identities.

The first quotient correspondence carrier is proof-only. `TerminalModule`
retains a strictly identity-ordered table for the narrow monomorphic, total,
direct faithful `define` certificate and the position-preserving direct
`lift` certificate backed by explicit `Congruence` and
`ForwardPreconditionTransport` evidence. The transport payload retains every
public-`Q`, representative-`P`, and congruence-legality fact's Left/Right
application side, authored source coordinate, and selected-theorem coordinate.
The codec serializes the complete
source-free certificate, including its canonically role-ordered theorem
evidence, and rederives its retained identity on decode. The role discriminant
precedes the selected application and role-specific payload in identity and
canonical bytes. Representation validation rejects missing, duplicate,
reversed, surplus, role/payload-mismatched, and unknown-tag evidence before it
independently reconstructs the theorem, correspondence, eligibility,
fact-major/source and theorem-coordinate order, exact congruence-`P`/transport-
`P` join, and direct-result shape. Format 53 / vocabulary 56 carry this
strengthened source-free contract. A nonempty table is still
rejected by execution validation, owns no machine or operation, and
does not authorize a representative call. The explicit producer attachment is
therefore a canonical-retention prerequisite, not executable quotient
lowering. A separate proof-only package-review row now covers the total, direct
`define` correspondence and the position-preserving direct transport-backed
`lift` by transactionally rederiving the complete source batch and retaining
the selected package's exact public callable, theorem/contract-fact
coordinates, relations, eligibility, and result coordinate. Package-review
schema 120 / row schema 78 / recovery schema 16 already encode the closed
transport kind and complete two-role payload, so no schema bump is required.
It is not ordinary checked package projection: quotient contract calls and
executable requests remain blanket-rejected, while two-argument lift, adapted,
literal, permuted, repeated, generic, private, and broader forms plus the full
package-review migration remain open.

Nominal static-machine callback bindings survive as explicit call-use rows.
Each row names the registration call and operation, static-machine argument
ordinal, selected machine and satisfaction row, exact canonical callback
requirement overload, and target entry recipe. The registrar's evaluated
outbound plan separately retains a fixed callback-materialization row from the
exact binder slot to one nominal native parameter or layout-field place. That
row's fingerprint is independent of the later selected callback; lowering
joins it to the per-use row and private thunk identity. It retains the
requirement's published envelope separately from the selected machine's actual
envelope and carries the checked actual-refines-published judgment. A native
address is never semantic input; only validated lowering materializes the
private relocation. Retained registration provenance keeps the selected
identity and lease disposition without making the actual envelope an ambient
caller fact.

A direct callback destination originates in an interleaved native-only
parameter on the registrar requirement. That declaration contributes no
semantic runtime parameter or address value. It contributes one ordered native-
telescope entry containing a compiler-issued nominal parameter identity, exact
binder/requirement source, and target-closed function-pointer shape. Ordinary
entries in the same telescope originate in semantic formals. Both
`NativePlace::Parameter` and `NativePlace::Field.parameter` index this one
identity space; the latter's root happens to be a semantic-formal projection.
The calling policy maps declared entries to placements and cannot create,
reorder, or retarget them.

Terminal identity retains two different hashes. The reusable physical
`CallPlan` fingerprint describes the ABI recipe. The boundary-plan application
fingerprint additionally covers the exact requirement, complete ordered native
telescope, every nominal parameter-to-placement row, and callback
materializations. This prevents an equally shaped parameter reorder from
replaying under an unchanged register sequence. Ordinal-derived IDs and the old
callback-placement fingerprint domain are retired through a versioned artifact
reissue, never reinterpreted as nominal IDs.

A nested layout-field place originates in one explicitly selected named
`PrivateCallbackSlot<Requirement>` conformance retained by the evaluated
layout plan. The conformance declaration is inert until the plan cites it; Psi
never reconstructs the slot through ambient conformance lookup. The retained
demand separates conformance-owned slot identity, exact target-neutral
requirement overload, target-closed physical placement, and complete layout
plan fingerprint. A byte offset may be authored inside the layout policy but
is neither slot identity nor a calling-plan coordinate.

Legacy native planning already binds each such row to the selected control-flow
entry and one deterministic private thunk symbol, failing closed if the entry
was lost. Each thunk now also owns one address-free callback-root schedule:
the exact canonical entry, activation-local runtime-flow/dispatch/storage/frame
identities, validated boundary entry plan, and its internal argument/result
bridge replay together before target instruction selection. Selection now
consumes that schedule for the first complete callback body class: a
zero-parameter, resultless, ordinary call-return terminal leaf emits one exact
private function with validated enter/leave mechanics. Its boundary footprint
is retained in canonical placement order under the callback function identity,
separate from the process-entry footprint, through machine emission. Callbacks
with parameters, results, body operations, transitions, or hidden semantic
state continue to reject. General multi-entry/re-entrant code emission and
placement of the private symbol into the registrar's declared native slot are
separate lowering steps. The first placement-planning step is address-free: it
keeps the handler's inbound entry plan distinct from the registrar operation's
outbound plan, retains the outbound plan's complete context and fingerprint,
and joins its exact binder/requirement/`NativePlace` row one-to-one and in order
with the emitted thunk/root schedule. Missing, duplicate, reordered, or
identity-drifted demands reject independently. The retained demand owns no
target operation, physical offset, bytes, object relocation, runtime storage,
native address, registration authority, or lease. Actual private-symbol
relocation still consumes only a complete validated outbound plan: missing,
duplicate, overlapping, shape-incompatible, or unresolved materializations
reject.

#### Retired custom/unknown carrier

An earlier custom/unknown host-operation prototype performed an address-free
registrar-occurrence and native-argument join after abstract lowering. That
carrier was removed during the canonical Terminal pipeline migration and is
not evidence for the current direct path. The replacement must enter native
realization as target-owned adjunct data keyed by stable Terminal
`OperationId`; canonical Terminal Psi itself remains target-neutral and source-
handle-free.

The target-closed backend recipe now extends that join to the exact outbound
parameter `ValuePlacement` and one authoritative private-layout demand for the
current single-slot `Field` form. Layout/slot/requirement/data-symbol identity,
offset, pointer extent, and alignment replay against the selected target and
containing data layout. The offset is retained evidence, not source-authored
identity. Multi-segment physical path composition rejects as an engineering
gap. The direct-parameter form is now owned by the separate canonical cohort
below; this paragraph describes the still-open projected-field form.
That field physical-destination recipe itself has no selected/assigned operation,
object symbol, relocation, bytes, runtime address, registration authority, or
lease; the exact registrar-native-parameter-to-assigned-operand join follows
separately.

The former custom/unknown assigned-operand carrier is likewise absent from the
canonical path. New work must not revive that source-specific branch. The live
direct-parameter cohort uses target-owned adjunct custody through assignment;
future work must consume that exact assigned row during thunk/address and
registrar-call emission with independent downstream replay.

#### Canonical direct-parameter custody

The authored direct-parameter path now reaches the preceding occurrence rung
on the canonical Terminal pipeline. Checked Unit plans retain the exact
statement/expression site for a bodyless boundary call only when every native
callback binder rejoins its admitted nominal selection. Terminal production
records the source coordinate, resolved registrar target, and emitted
`OperationId` ephemerally; the compiler consumes that source-bearing join and
retains a placement-index-to-`BoundaryCall` row in the target-owned native
proposal. The proposal also carries the exact target-closed direct parameter
application: nominal `NativeParameterId`, authored ordinal, function-pointer
shape, and plan-selected `ValuePlacement`. Retained-product replay compares the
complete row with the independently validated callback placement. Canonical
Terminal Psi remains source-handle-free. Proposal replay rejects missing,
duplicate, unreachable, wrong-target, non-boundary-operation, native-
application, and artifact drift, while native realization remains fenced. The
first consuming rung is now closed for exactly one direct
`NativePlace::Parameter` callback on the ordinary unoptimized normalized-import
path for the current x86-64/AArch64 native targets, with only the established fixed-width
integer semantic arguments and result. It requires exactly one binder, demand,
and materialization, a target-pointer-shaped application, and one complete
register or stack placement. Target lowering joins the proposal to the
exact retained registrar `BoundaryCall` by `OperationId`, carries the selected
callback-thunk `MachineFunctionIdentity` and exact native-parameter application
at its authored ordinal beside the ordinary source scalar arguments, and
physical assignment preserves that tuple while binding the plan-selected
parameter destination. The callback contributes no Terminal operand, source
`ValueId`, Omega runtime type, inferred position, or trailing argument. Field
destinations and multiple callback entries remain fenced. This remains
compiler-owned target custody: Terminal Psi does not encode the thunk identity
or physical assignment, and the source-free artifact cannot reconstruct either
from the operation alone. A separate retained-product row now owns the exact
checked callback body as an isolated canonical Terminal artifact. Native
realization independently recompiles it into a disjoint compiler-private
machine-code function; object construction binds its exact placement-derived
symbol and final-image replay requires one matching executable region. The
callback artifact's local `MachineId` remains nested and cannot impersonate a
semantic program function. Machine emission materializes the private address at
the authored native ordinal; object construction targets the exact private
symbol with architecture-specific relocations, and final-image replay decodes
the patched address and requires the private function's final text address.
The source-evaluated import route rejoins the callback-closed registrar plan to
the exact normalized locator and explicit receiving policy before this work.
No runtime registration, invocation, installed-address lifetime or lease,
installation, or publication authority follows.
The application-v3 commitment travels as a compiler-origin provenance
projection; target lowering cannot rederive it because the complete authored
nominal telescope is intentionally absent after source-free handoff. The exact
placement-to-`OperationId` mapping is likewise producer-owned. Retained replay
requires unique boundary-operation identities but cannot reconstruct the
authored site from Terminal Psi alone. Independent commitment authentication
and an independently replayable site-to-operation receipt remain later
publication prerequisites.

#### Retired object-store and installation prototype

The removed custom/unknown carrier used a field/BSS object-store request,
`RuntimeStorageAddress`, compiler-inserted address-store operations, and a
callback-specific installation manifest. None of those carriers is part of the
current canonical direct-parameter path, and no surviving task may cite them as
runtime registration, lifetime, or publication authority.

The current path instead passes the callback address directly in the declared
native parameter. Object and final-image validation bind the relocation to one
compiler-private function. Canonical installation format 51 then retains that
function's exact private identity, source-Psi identity, artifact-local machine,
fixed-integer ABI, and final text interval and rejoins the decoded record to the
same executable image. This remains non-authoritative installation evidence;
it creates no installed entry, resolved address, registrar result, external
root, `Registration`, capacity, lease, or publication authority.

Provider-execution metadata crossing target operations, machine code, and the
native artifact is an authority-free report projection. Its compact selected
plan, execution, normalized-root, and boundary-contract coordinates are named
report identities/fingerprints and preserve the existing installation wire
order. The admitted provider object borrowed at the lowering entrance remains
the authority carrier. The retained artifact additionally keeps the strong
selected-provider-closure digest and exact requirement strings/catalogs, and
its replay rejects exact-requirement substitution even when all compact report
coordinates are unchanged.

The canonical checked-to-Terminal function keeps target-owned callback data out
of Terminal Psi. While checked source and the canonical artifact coexist, the
compiler product driver joins each validated placement to one emitted
`BoundaryCall` `OperationId`, then retains the occurrence and direct native-
parameter application in the native-realization proposal beside the artifact.
Terminal artifact production is live. Native realization consumes and replays
the sidecar through target operations, physical assignment, object/image
relocation, and canonical installation custody; it may not reconstruct the
authored callback join after handoff. No callback-specific deployment
pending/live carrier currently exists. Generic installed-code and external-root
primitives survive, but the registrar-result, capacity, lease, source
`Registration`, and transactional retry joins remain open engineering work.

Reference identities retain loan compatibility and permitted operations
separately. `&write T` carries an exclusive loan over an existing valid `T`
with mutation but no observation authority. Terminal production preserves that
mode through calls, projections, reborrows, provider selection, and ABI
lowering; it may not serialize the physically identical pointer as ordinary
read/write authority. Content-independent place projections and metadata reads
remain available, while loads, readable reborrows, takes, swaps, and
read-modify-write reject.

The August 2026 checkpoint retains this access mode through checked
whole-value and fixed-byte-element replacement, including dynamic indexes whose
ordinary range obligations are proven, plus unrestricted primitive-leaf stores
through exact finite common-field paths of plain invariant-free records. Nested
record writes retain every field identity; dynamic indexes remain conservatively
collection-wide in caller-visible mutation summaries. A forwarding-only
Terminal rung first carried closed owned/shared/mutable/write-only access on
structural parameters and call arguments in canonical format 27.
That rung now includes one exact unrestricted `WriteOnlyBorrow` field-path
subloan from a mutable or write-only borrowed root. Attenuation preserves the
root's declared access and records `WriteOnlyBorrow` on both the operand and
callee; it does not permit readable access within the callee.
The verifier replays its ordered path, structural type, and access and
treats it as a claim-free non-transferring subloan rather than an owned linear
projection; malformed path, target type/access, source access, qualification,
arity, or provider substitution rejects. Reusable local or re-entrant reborrow
authority does not follow. One direct write-only literal fixed-array parameter
root may carry a finite nonempty suffix of ordered in-bounds `FixedIndex`
segments through recursively literal fixed arrays, either directly or after
the eligible field prefix, when the ultimate element is an unrestricted
non-Atomic primitive. An implicit write-only receiver additionally admits
material record leaves through finite interleaved field and literal-index
paths. The source producer retains parameter or attached-self roots and exact
callee identity; independent source replay rejects even an in-bounds index
substitution. The verifier independently rejoins every array shape, bound,
element type, multiplicity, and write-only access. Indexed receiver roots must
have closed material record/array/scalar structure, without erased fields,
descriptors, or sums. Dynamic and range paths, whole nested-array leaves, and
retained local receiver aliases remain outside this producer. Canonical
receiver artifacts execute under incremental fuel without repeating calls or
stores. The verifier also rejects widening, target disagreement, overlapping
exclusive arguments, and Boolean structural observation through write-only access.
For direct Unit calls, target lowering walks the same finite field/index path
and derives one borrowed-reference pointer adjustment. Both Linux targets emit
that adjustment, including split AArch64 immediates beyond 4 KiB; assignment,
object, and installed replay independently reconstruct the exact offset from
the retained structural declarations. Literal indexed Unit receivers use this
same route for material record leaves, including interleaved fields/indexes and
mutable-root attenuation to write-only access. The root and leaf retain
`BorrowedReference` shapes; the argument adjusts the original pointer rather
than staging a referent copy. Wrong paths, offsets, types, access, or value-copy
shapes reject independently during assignment and object/installed replay.
A checked scalar prefix may coexist with that projected argument and is replayed
through its ordinary scalar ABI custody rather than folded into projection
authority. The legacy fixed-array length/stride
rows remain reserved for affine element-transfer cleanup and are not attached
to this unrestricted write-only subloan.
Terminal format 42/vocabulary 45 retains one executable direct whole-root
primitive replacement from a landed integer literal, landed IEEE float
literal, or Boolean literal. `PrimitiveScalar` is an honest structural
referent shape rather than a synthetic record, and
`WriteOnlyPrimitiveStore(destination, value)` is Unit, unconditional, and
non-observing. Verification requires one claim-free, unqualified, unrestricted
`MutableBorrow` or `WriteOnlyBorrow` parameter root and an already-defined
exact-type SSA value. The operation name describes its non-observing effect,
not a requirement to discard readable authority. Reference execution keeps
exact-typed primitive backing outside suspended call
frames, so a callee replacement is caller-visible; fuel is consumed before the
mutation and resumption cannot replay it. Broader projected/aggregate stores
and opaque-provider realization remain gated, except for the exact plain-record
field lane described below. The current native whole-root
lane selects a borrowed-reference ABI on x86-64 and AArch64. Fixed integers
retain their exact referent width/alignment, declaration, placement, immediate,
and operation identity. Booleans retain a distinct one-byte referent, preceding
Boolean definition, and literal rather than masquerading as integers. Physical
assignment independently replays both forms. Machine emission rejoins the
destination, type, placement, source, and exact parameter home before emitting
the target store; the Boolean source additionally retains its definition
ordinal. Dedicated whole-root records bind these facts to exact code intervals
and bytes. Object construction independently replays the declaration joins,
borrowed-reference placement and home, literal source, architecture encoding,
attribution, interval, and bytes. The current installation format transports and
revalidates the integer, Boolean, and IEEE float store families without
treating physical pointer-layout equivalence as permission equivalence. IEEE
float stores retain their exact preceding definition, raw bits, referent shape,
store-only source carrier, machine bytes, and attribution through both object
and installation replay.
The semantic store lane also admits one exact scalar parameter from an ordered
scalar roster as the replacement source when it matches the primitive referent
exactly. Checked planning retains the authored
structural/scalar parameter partition; Terminal verification and target-neutral
abstract lowering preserve the runtime value identity without fabricating a
constant. The native sublane admits every non-address fixed integer
(`i8`/`u8` through `i64`/`u64`) plus Boolean and independently replays its exact
type, selected incoming ABI placement, destination home, machine bytes,
and installation source record on both Linux targets. Terminal format 77/vocabulary
80 now retains an ordinary
`CallUnit`'s scalar arguments independently from its structural arguments,
verifies their exact arity and types against the callee, transports them through
canonical bytes, and binds them in reference execution. This admits a semantic
caller that forwards an ordered scalar roster across both register and stack
argument locations into the
write-only-store callee, including a callee store sourced from a later roster
position.
Abstract lowering and optimizer identity/dataflow retain and revalidate the
same scalar arguments. Abstract-to-Target lowering now derives one complete
call plan with a scalar prefix and structural suffix while preserving the exact
caller source. Target-to-assigned lowering now independently replays that plan
and binds the caller's scalar parameter to its exact incoming physical
location. Machine emission admits the bounded same-register transfer and exact
incoming-stack load/outgoing-stack store, rebasing the incoming slot across the
Unit frame and transient call area before emitting the structural suffix and
call. When authored argument order would overwrite an incoming scalar register
that remains a source, emission first snapshots the complete register-source
set into deterministic compiler-private slots in that same transient area.
Arguments then materialize in semantic order, covering cycles, duplicate
sources, register-to-register, register-to-stack, and stack-to-register
transfers without relying on sequential-move luck. The callee may likewise load
the selected stack-carried scalar and store it through the staged write-only
destination. Records retain every scalar source, destination, interval, and
relocation coordinate. Object construction
independently rejoins the Unit callee ABI, caller parameter source, frame and
call-stack evidence, structural copy, and exact bytes. The current installation format
transports and replays the same custody on both Linux targets. The ordinary
call source may be either the caller's exact native-fixed-integer-or-Boolean
parameter, an exact preceding Boolean literal definition, or the durable home
of one exact preceding ordinary fixed-integer scalar call. The direct-store
sibling also accepts one exact selected boundary-operator realization. These
forms are rooted in an explicit checked `ScalarCall` or
`SelectedOperatorScalarCall`: checked-to-Terminal lowering rejoins the original
application coordinate, target state, full contract commitment, service reach,
and every scalar argument fact before adding its scalar graph to the attached
closure; the selected form additionally rejoins its requirement and exact
ProviderPlan. Target assignment gives the result an ordinary durable home;
later `CallUnit` assignment may source an ordinary-call home across register or
stack boundaries, and machine/object/installation replay independently rejoin
the home and materialization bytes before the callee stores it. Further
arithmetic locals, Boolean and IEEE call results, IEEE runtime sources, and
non-native integer carriers remain fail closed.
The same checked scalar-result binding may instead feed the immediately
following whole-root primitive store directly, without introducing a helper
Unit machine or call. This exact sibling requires one ordinary service-free
fixed-integer producer or one selected scalar-only checked-body realization,
one immutable dense local, one matching unrestricted mutable or write-only
primitive destination, and no other parameters or body work. Terminal retains
the call result as the store's SSA value. Target and
assigned custody preserve the exact durable home, machine emission loads that
home and stores only the referent width, and object/installation replay rejoin
the unique earlier call, unique home, exact bytes, and attribution on both
Linux targets. The reference interpreter executes mutable and write-only
destinations with the same non-observing store semantics.

Terminal format 61/vocabulary 64 adds the first source-produced projected
scalar replacement needed by a closed named-dynamic call. The checked plan
retains one exact literal assignment into the selected carrier immediately
before binding and invocation. `StructuralScalarFieldStore` then carries the
mutable or write-only destination parameter, the exact path to the carrier
record, its relevant scalar field ID, and the already-defined typed value;
authority remains on the complete parameter declaration. The selected
realization observes an exact direct `i32` leaf through
`IntegerStructuralField`, while the older Boolean observation remains a
separate operation. Verification independently rejoins the unrestricted
parameter row, readable or writable access, empty qualification/claim custody,
record path, relevant field, scalar type, and dominating SSA definition.
The path may contain any finite nonempty-identity sequence of relevant record
fields; fixed-index, case, erased, missing, and scalar-intermediate segments
reject. Each consumer walks that exact sequence rather than trusting a
producer-supplied accumulated offset.
Canonical encoding, semantic observation, fixed fuel, and reference execution
preserve the store across the projected call without asserting a native
layout. Terminal-to-abstract lowering retains the complete rows, and optimizer
validation reconstructs their definitions, uses, places, and provenance. The
native scalar lane realizes the exact direct integer-field read and the bounded
projected immediate store/call sequence on x86-64 and AArch64. Target lowering
retains the complete store and call rows; physical assignment independently
reconstructs the field layout, preceding integer source, argument ABI, and
projected copy; machine emission records the exact parameter home, code
interval, and bytes. Object construction replays the architecture encoding,
and installation format 51 transports every store field and rejects semantic,
source, home, interval, or byte substitution. The ordinary production compiler
uses this lane when no optimization is selected. General mutable-borrow
writeback still requires complete native structural-parameter identity
enforcement; the direct fixture observes its store in the
following call before returning. `WriteOnlyPrimitiveStore` remains the distinct
whole-root operation.

The same `StructuralScalarFieldStore` vocabulary now carries an ordinary
attached-Unit replacement with no dynamic dispatch. Source checking rejects
assignments and explicit exclusive field arguments through a shared or absent
state receiver; attached fields do not become writable merely by appearing in
the machine namespace. Its checked source is one
fixed-integer or Boolean literal, one exact same-typed scalar parameter, or the
fixed-integer result of one immediately preceding ordinary scalar call or
selected boundary-operator realization through the sole
unrestricted-or-affine mutable/write-only record parameter or receiver,
with an exact
field write frame or the indexed form's sole conservative containing-array
frame and no claims or qualifications. An empty carrier path denotes a
primitive field directly on the root record; a nonempty path names each
enclosing plain record field before the final scalar field. One bounded sibling
ends the carrier path with one in-bounds literal `FixedIndex` into a closed
material `[copy]` record array and then names the final scalar field separately.
Checked-to-Terminal lowering shares the dynamic realization path/type replay
rather than maintaining a second field walker. Abstract and target lowering
independently reconstruct direct and nested byte offsets, and assignment,
machine emission, object construction, and installation replay preserve the
exact parameter home, scalar parameter location or durable result
home, and store bytes on x86-64 and AArch64. Multiple writes, observations,
delayed results, arithmetic locals, nested or dynamic indexes, ranges,
aggregate array elements, cases, erased fields, constrained data, and opaque
shapes remain fail-closed.

Receiver stores resolve source `Self` through the machine's exact attachment
declaration and retain `is_self` at structural position zero. Their write frame
uses the receiver root `self`, not the ordinary parameter root `$P0`; neither
an opaque frame nor a substituted field/root authorizes a literal store.
Callable receiver storage and executable entry provisioning are separate:
direct native executable realization rejects a retained receiver entry until
the generated bridge supplies its root-backed, initialized activation loan.
An attached namespace with no runtime receiver parameter needs no such pointer.

Literal indexed write-only receiver regressions execute complete relocated text
with caller-owned storage and verify the selected write and unchanged surrounding
bytes after return. This covers register-passed references, nested/interleaved
paths, mutable-root attenuation, and scalar replacement prefixes on macOS
AArch64; Linux-host and stack-passed receiver execution remain unfinished.
It does not establish general structural-borrow identity. The [structural borrow identity contract](../../design_briefs/core_multiplicity_and_linearity.md#structural-borrow-identity-at-native-calls)
is settled: every borrowed mode preserves the original referent, using the
existing reference ABI tag without erasing semantic access. Native structural
signatures must derive through one access classifier; caller argument
preparation and receiving validation/replay independently preserve that
relationship. Shared-borrow classification and complete producer/boundary
enforcement remain unfinished. A pointer staged in a local home is not an
object copy, and a direct-copy fixture is not proof of reference semantics.
Terminal interpretation does preserve the opaque referent identity and indexed
path across calls and returns. Shared projected arguments from an unrestricted
root retain unrestricted multiplicity through record-field paths, optionally
followed by one literal index; indexing does not convert the referent into a
linear value. Deeper shared array projections remain unsupported. Ordinary access
checks still reject readable subloans from write-only roots. Interpreter
regressions write distinct array elements in a write-only callee, read each
after its return, and pause at every operation/return fuel boundary to check
that committed writes are not replayed. This artifact-level execution coverage
does not expand the checked source producer's single-store body grammar.
Ordinary single-state Unit source can forward a shared field or literal-indexed
record element through the same rule. The caller and callee each retain one
structural parameter, unrestricted referents, empty qualifications, and exact
projected type/access; parameter-dependent contracts and content evidence
remain outside this source-call form. Canonical reload and interpretation use
the source-produced argument path without frontend state.

Terminal format 62/vocabulary 65 carries the bounded one-reassignment dynamic
scalar lane without devirtualizing it. The caller owns two dense conformance
selections, one descriptor naming their initializer/latest relationship, one
indirect-dispatch row naming the exact closed row and realization, and a
`CallDynamicScalar` operation that contains only the descriptor ordinal plus
ordinary obligation/crash lanes. The latest selection supplies the runtime
shared structural source; the operation contains neither a static callee nor a
source copy. Independent validation rejoins both sources to the same closed
application and structural type, validates each against the realization's
single structural parameter, requires exact row/callable/result/service reach,
and rejects orphan, duplicate, reordered, retargeted, or direct-call-
substituted custody. Canonical encoding, call composition, call-graph/service
reach, reference execution, and fixed-fuel derivation all resolve the same
descriptor table. Terminal-to-abstract lowering rejoins those exact rows into
one target-neutral dynamic operation. Target legalization derives both source
copies and the selected realization's structural-argument/scalar-result ABI.
Optimizer reconstruction and independent validation retain the dynamic result,
both declared source places, descriptor-version relation, selected-realization
call graph, service closure, and non-rewritable call observation;
physical assignment allocates the canonical runtime `{ instance, table }`
carrier and a durable scalar-typed result home. Machine-emission preflight independently
replays those non-overlapping descriptor/result frame regions. Each abstract
dynamic carrier retains the complete canonically ordered
`ClosedConformanceApplication`, not merely the selected indirect row.
Optimizer identity binds the full telescope, row map, callable registry,
report coordinate, and strong commitment; optimizer validation, target
lowering, and assignment rejoin that application to the exact selected row and
callable. Multi-row mutation tests reject removal or reordering of an
unselected row. For a service-free straight-line Unit body, machine emission
now materializes the canonical `{ instance, table }` descriptor and an actual
indirect call on x86-64 and AArch64. Object construction deduplicates
semantically identical complete applications by strong commitment, emits one
canonical data slot per row, and binds every row with an executable callable
to its exact function symbol. A retained semantic row without a callable
remains a zero trap; it is not deleted, collapsed, or assigned an invented
function. Independent object and final-image replay validates descriptor
bytes, table-address text relocations, the selected-slot load and call, exact
function-slot relocations, and relocated final data. The canonical installation
record retains the final text/data layout, initialized-data commitment, complete
table/slot projection, and each rebound call's application, sources, selected
slot, realization, and text interval. Runtime replay preserves the target
section gap in one frozen extent, reapplies sealed entry/data relocations at the
retained final addresses, and binds both pre-relocation and final text/data
bytes to the installed occurrence. This is the bounded immutable-table lane,
not general mutable initialized-data or BSS support. Multi-block continuation
remains independent unfinished work. None of these boundaries permit
devirtualizing the call.

Terminal format 70/vocabulary 73 introduced result-less dynamic Unit dispatch without
inventing a scalar carrier. A direct local selection remains an exact
`CallUnit` joined to its direct dispatch row. A same-conformance rebound uses
`CallDynamicUnit`, and an exactly once-rebound descriptor passed through one
transparent helper uses `CallDynamicParameterUnit`. Both explicit dynamic
operations have `OperationResult::Unit`; their descriptor/parameter ordinal,
requirement slot, obligations, and crash continuations round-trip canonically.
Validation rejoins the complete application, Unit callable interface, selected
row or parameter slot, operation, access, and source. Fixed fuel and reference
execution resolve the same rows without allocating a value ID or result home.
Format 71/vocabulary 74 added a distinct owner-local selection argument source,
retained by current format 79/vocabulary 85 alongside rebound descriptors and
inbound parameters. Direct-selection
Unit and scalar forwarding therefore cross the helper without relabeling their
custody. The scalar form retains its exact result through the ordinary caller,
parameter-slot helper, target assignment, durable Unit-frame home, and native
artifact replay; the Unit form retains no result carrier. The
Terminal-to-abstract boundary retains both dynamic Unit variants without a
scalar definition. Local rebound dispatch and the exactly-once forwarded form
now cross target lowering, assignment, native emission, object/image replay,
and installation. The forwarded form remains two explicit roles: a result-less
outer descriptor call and a result-less helper parameter-slot call.

Scalar-result rebound descriptors may select a different named conformance at
the reassignment while preserving the exact carrier, dynamic-trait interface,
borrow access, telescope, and ordered requirement roster. Terminal Psi retains
the initializer and latest selections against two independently committed
closed applications. Only the latest application owns executable callable rows
and supplies the runtime table; the already-overwritten initializer application
is retained without inventing an unused table. Verification rejects interface
drift, missing applications, or collapsing either selection onto the other's
commitment. This works for caller-local scalar and Unit dispatch and for both
forms through the existing transparent descriptor-parameter hop. Unit remains
result-less throughout.

The same closed application now admits one exact mutation-bearing callable in
Terminal form. Its checked row carries one, two, or three distinct ordered
primitive-field literal stores through `&mut self` separately from the scalar
return expression. Each field may be directly below the realization's attached
self type or below an exact finite path of relevant named record fields.
Lowering materializes every constant and store before the existing self-field
read, and Terminal validation retains and replays every carrier-path segment
and source order exactly. An exclusive field
subloan from an unrestricted mutable-borrow root retains unrestricted
multiplicity for the duration of that exclusive call; it does not invent a
linear entry claim. Direct Boolean and signed or unsigned 8-, 16-, 32-, or
64-bit integer literal stores now lower
`&mut self` as one no-copy pointer to caller storage, emit the stores before an
independently identified direct scalar field read, and retain independent
x86-64/AArch64 machine, object, image, and installation replay evidence. Target
lowering accumulates every record-field and final primitive-field offset;
assignment replays the path against the retained declarations, and machine
emission rejects path, offset, or order drift before producing bytes. Canonical
installation format 68 retains the bounded ordered store vector. A
structural-only scalar-result realization publishes the ABI that its private
erased-data adapter must rejoin before calling it. Boolean-returning forwarded
calls now publish the same ABI with an exact Boolean result, normalize and
store that result into a one-byte durable Unit home, and branch directly on
that home with target-specific zero tests. The first Boolean store still
returns an independent `i32` self field through the fixed-integer lane. The
same ordered carrier retains a third store. A fourth write, repeated
destination, indexed/case projection, address or IEEE-float
literal, computed store value, shared receiver, or body reorder remains outside
the native carrier.

Each write-only event names its exact loan occurrence, projected logical place,
physical write footprint, and outcome guard. Verification invalidates facts
only on written paths and preserves facts over an explicitly unchanged suffix.
It also reconstructs freely discardable displacement and post-write validity
from static structure, written inputs, and explicitly supplied premises; no
premise may originate from observing the referent. For a checked realization
the restriction is derived from its call closure. For an opaque realization it
is an admitted provider judgment, pinned to the selected implementation and
receipt, unless installation supplies physical isolation evidence.

### Borrow-compatibility certificates

Ordinary borrow admission ultimately crosses Terminal as two joined but
independent rows. The resource row retains each loan occurrence's exact owner
lineage, captured place, access polarity, parent lifetime, and restoration
obligation. A proposition-derived compatibility row retains the loan-formation
event, captured loan/place identities, normalized relational conclusion, exact
premise fact tokens, and proof derivation.

The first checked-only precursor is now retained before this Terminal crossing.
For every loan/loan pair admitted by the existing automatic non-interference
judgment, a separate proof arena records a zero-premise `Structural` row with
the exact machine/state/statement formation coordinate, two state-owned loan
handles, frozen captured places, and normalized relational conclusion. Repeated
checked-fact validation rebuilds that arena deterministically, and resource
rejoin rejects changed handles, places, or formation coordinates. A separate
formation snapshot retains every consulted dynamic-selector coordinate in
ordered forming/active path position with its normalized integer,
immutable-symbol, or conservative-unknown result. Checked replay independently
normalizes the exact typed formation expression, requires equality with the
snapshot, recomputes spatial disjointness, directed containment, and
access-aware non-interference from the frozen places plus authoritative loan
polarities, and requires the zero-premise `Structural` derivation. Missing,
reordered, malformed, or conclusion-changing selector rows and duplicate or
stale certificate roster entries reject. This carrier does not yet encode
Terminal evidence or admit proposition premises, and it does not alter the loan
resource rows or admission semantics.

Checked loan formation also retains one narrow parent-lineage prerequisite.
Every loan is classified as a direct root, a direct reborrow naming one exact
parent loan, or an unretained derived occurrence. Only an explicit
reference-local reborrow with exactly one prior matching state-owned loan may
name a parent; checked replay reconstructs that source occurrence, owner path,
formation order, and rebased captured place. Multihop chains name immediate
parents. A separate checked-only arena now closes the narrow direct-reborrow
case over the child's exact state/owner/place/access and activation/weakening
lifecycle. Each row has a typed handle to either the direct-root parent
resource or an earlier reborrow resource; transactional replay validates the
whole graph before rebuilding and remapping those handles in loan order. Its
restoration member is a pending child-to-parent obligation, not evidence of
parent activity or reactivation, temporal containment, or completed
restoration. Compatibility certificates must rejoin these exact child rows.
Aggregate and helper transfers, ambiguous or reassigned aliases, and
write-only local loans remain fenced with no resource row. Neither checked
resource arena supplies Terminal authority.

Every retained direct child also carries one checked parent-suspension
formation boundary. It names the exact child activation plus the unique
parent-loan constraint in that statement's entry set and rejoins both to the
existing child and typed parent resource identities. Transactional replay
rejects missing, duplicate, substituted, reordered, or cross-state
occurrences. This proves only that the parent occurrence was available
immediately before the explicit child formed. It does not prove continued
parent activity, suspension-interval containment, reactivation, completed
restoration, or Terminal authority. The lexical parent weakening may precede
the child weakening, so those later lifecycle claims require a distinct flow
carrier.

One further checked-only join retains the exact parent and child weakening
handles and classifies their semantic boundary order as parent retired before
the child, retired with it, or remained lexically live past it. Statement
expiry precedes entry, local reassignment ends the old carrier after its
right-hand side, and state exit is last; raw arena order has no meaning.
Independent replay checks both handles, the existing resource identities, and
the derived status. This is lexical disposition only, not authority return,
reactivation, cascading restoration through retired parents, suspension
containment, or Terminal evidence.

Checked resource replay now also merges activation and weakening facts into
semantic phase batches under the exact nine-cell parent/child access relation.
A shared child of `Read` merely releases; shared descendants of `Mutable` form
a cohort that freezes mutation and restores `Mutable` once after its final
descendant ends; a permitted exclusive child suspends one exact branch and
restores the parent's original access. `WriteOnly` can produce only
`WriteOnly`, while `Mutable` may attenuate to either `Read` or `WriteOnly`.
Exclusive lineages close deepest-first, shared cohorts release as a set, and
retired carriers retain their exact ordered weakening path.

Same-boundary lineage closure and state-exit direct-root handoff are now
distinct checked-only outcomes. The former requires the final retained carrier
to end at the child's exact semantic boundary; the latter requires `StateExit`
and an exact direct-root-lifetime target. Independent replay reconstructs that
phase/path/target conjunction and rejects swapped labels transactionally. No
checked disposition is Terminal authority by itself. Terminal may publish
restored use or root custody only after independently replaying exact lineage,
polarity, semantic boundary, projection, and suspension/freeze-containment
evidence.

The checked side now retains that containment evidence in a separate sibling
arena after its complete lifecycle replay succeeds. Each permitted exclusive
child binds one suspension row, each `Mutable`-to-`Read` child binds one freeze
row, and `Read`-to-`Read` release binds none. A row rejoins exact child and
typed parent resources, both accesses and their classified effect, activation
and parent-entry formation identities, both weakening identities, and frozen
parent/child places plus the exact projection remainder. Missing, duplicate,
reordered, amplified, or retargeted rows reject transactionally before resource
rebuild. The checked certificate is non-authorizing, and reaching a root grants
no cleanup, transfer, or linear-discharge authority to the borrow layer.

A sibling checked post-restoration row now retains the first exact later use.
One direct mutable parent lends either an exact mutable/write-only exclusive
child, one exact shared child, or one exact two- or three-member concurrent
shared cohort. Other non-overlapping sequential exclusive siblings may occur.
The exclusive form binds matching `Reactivate` and `ExclusiveSuspension`; the
shared form
binds `RestoreSharedCohort`, exact roster `[child]`, `[left, right]`, or
`[left, middle, right]`, and one independently replayed `SharedFreeze` row per
member. A multi-member form makes every final use in one exact empty,
receiver-free observation call with one shared parameter per member. Its
borrow-only aliases erase to the same certified parent
place, and the whole-parent mutation is the immediately following statement.
All admitted forms end by `LastUseExpired` before one
runtime-receiver-free call with one exact mutable-reference parameter over the
bare parent carrier whose mutation summary covers the complete restored
referent. Nominal static qualification such as `Sink::mutate(parent)` is not a
runtime receiver. Transactional replay
rejoins both resources, weakening, disposition, containment, flow and borrow
calls, carrier-read access, parent-loan entry constraint, captured places,
restored access, and target identity. Four-member or sequential shared,
multihop, other concurrent-sibling, state-exit, projected, receiver,
extra-parameter, direct-assignment,
partial-mutation, and nonmutating shapes remain absent. Terminal independently
replays the checked join, the caller's exact `CallUnit`, and the receiver-free
callee shape, then publishes one canonical row binding that operation to its
ordinal-zero source call coordinate, bound callee, direct-root lifetime,
explicit restoration class, child interval, and exact shared-cohort roster.
The canonical target identity is committed source custody, not an identity
reconstructed from machine bytes. The producer transactionally proves the
selected checked cohort complete; the verifier replays exact roster cardinality
and member equality and requires the shared caller to contain exactly one
compatible whole-parent mutating `CallUnit`. Authority is limited to the
named call; cleanup, transfer, and linear discharge remain unexpressible.

The bounded Terminal publication now consumes that complete checked join for a
finite nonempty linear exclusive lineage rooted in one direct-root `Mutable`
loan and ending at state exit. Each root-to-leaf edge is exactly
`Mutable`-to-`Mutable`, `Mutable`-to-`WriteOnly`, or
`WriteOnly`-to-`WriteOnly`; one-hop publication remains the length-one subset.
The producer reverses the leaf's exact retired-parent path, rejects branching,
and rejoins every resource, activation, parent-entry formation, weakening,
captured-place, projection-remainder, and `ExclusiveSuspension` row before
accepting `StateExitDirectRootHandoff` to the exact root lifetime. Format 53 /
vocabulary 56 round-trip the complete lineage and restored-use publication.
Verification rejects empty,
reordered, duplicated, access-amplified, malformed, redirected, branched,
shared, or non-state-exit rows. Publication remains direct-root custody only:
there is no cleanup, transfer, or linear-discharge vocabulary. Three-member or
sequential shared restored-use cohorts, multihop or branching restored use,
projected or direct-assignment restored
use, non-state-exit root custody, and broader branching remain outside this
rung.

The row does not serialize "dominates" or "is valid" as trusted claims. The
verifier reconstructs control-flow dominance and path availability from the
premises' establishment points, checks their exact value/place versions and
validity scopes at formation, replays the derivation, and confirms that its
conclusion names the places actually captured by the resource rows. Premises
may expire after formation: a borrow captures a place rather than a live index
expression, so compatibility over the frozen loan occurrences survives without
retargeting either loan.

Relational evidence never repairs a missing resource row or widens its access.
Conversely, a live exclusive authority does not prove that two projections are
disjoint. A rule such as "no conflicting writer" is accepted only when the
resource ledger supplies the write loan and the compatibility certificate
proves non-interference. Circular justification rejects before either row can
authorize execution.

Logical place footprints in these rows are not semantic `Content<A>`
projections or backend physical effect footprints. Terminal retains explicit
checked bridges when a particular carrier or operation relates those notions;
it never identifies them by shape.

Normalized logical expressions retain exact binders, subjects, substitutions,
and dependencies. Presentation names do not define truth or authorize proof
reuse. Ordinary trait/conformance bundles retain their complete mathematical
interface separately from the statement and its derivation provenance.

The contract/bundle migration must carry exact witness identity through
forwarding and repeated projection. Different witnesses proving the same
statement do not become one value. Closed conformance selection retains its
complete telescope, trait application, and normalized row map; no expected
shape, display name, or visible-fact search selects a missing implementation.

Guarantees remain path-sensitive. Every ordinary exit must establish its
applicable conclusions. Caller import requires the matching result case and
the exact argument/result substitution. Validity is the intersection of
referenced occurrence and bundle scopes; intersecting writes invalidate
borrowed or revisioned facts. A bundle cannot make a conditional fact available
unconditionally.

Proof-only data adds no runtime storage, call, or fuel. Evidence accompanying
an executable call must remain bound to that exact operation and callee, result
shape, and specialization, not a second inferred invocation. Independent replay
rejects missing laws, orphan witnesses, changed substitutions, stale scopes,
or runtime links that do not match. Transitive assumptions remain separate
from the proved statement and subject to the receiving policy.

These are the target preservation requirements, not a claim that the current
codec implements the replacement. `PROOF-CONTRACT-MIGRATION` in
[TASKS.md](../../../../TASKS.md) owns the source, representation, codec, and replay
migration under the [mathematical proof contract](../../design_briefs/mathematical_proofs.md).

Omega task activation applies the same authority split after checking.
`TaskRuntime::{start,try_start}` retains its compact specialization value only
as a report coordinate; provider planning derives a domain-separated strong
commitment over the exact checked TaskRuntime requirement and operation, exact
package-qualified target/entry signature including parameter modes, and target
machine-contract commitment. The task runtime receipt binding carries both
values but derives invocation identity from the strong commitment alone, so
compact equality never authorizes a different specialization.

Trait requirement calls retain the public callable contract separately from
the selected private realization. Replaying the closed conformance must join
that exact public requirement, owner-scoped application, realization identity,
and ordinary call. A private strengthening cannot become a public guarantee.
Evidence must not select a different executable lowering family or replace
independent validation of the call and its result.

Relation applications retain their independently bound left and right carrier
index packs; no global carrier-parameter role is serialized. Selected
constructor lifts, dependency-ordered field relations, and every required
proposition-transport proof enter the semantic rows that justified a lifted
operation. Callable argument telescopes use positional identity, with source
parameter names confined to debug metadata.

An erased Type binding remains in typed semantic and proof rows with its
multiplicity, validity scope, conservation obligations, and provenance. A Prop
binding is copyable and retains its validity scope and provenance without a
usage-count obligation. Neither has an executable storage place or cleanup
action. Runtime layout and operation encoding consume the erased-stripped form,
while semantic fingerprints retain the binding and its type.

Unit structural declarations apply the same rule directly: every field row
retains authored relevance, and an erased row carries its exact normalized type
identity as an opaque semantic type rather than forcing proof data into the
executable structural-type graph. The codec and verifier reject mismatched
relevance/type rows. Omega skips erased rows before ABI classification, so the
terminal artifact preserves semantic identity without assigning proof evidence
an offset or transfer.

An entry claim may name its complete structural parameter or a typed path below
it. Record segments use the field's exact canonical identity: `#<id>` for an
authored numbered field and its spelling for an unnumbered field. Literal array
segments carry their canonical zero-based index and resolve only through a
nonempty literal-length fixed-array shape. A projected claim is linear even
when its containing aggregate is affine. Paths traverse only relevant
structural fields or in-range fixed indexes; cases, dynamic indexes, scalar or
erased leaves, unknown segments, duplicates, overlapping ancestor/descendant
rows, and noncanonical order reject. Direct Unit calls require the caller and callee to
agree on the complete ordered claim-path set for each structural argument, and
content-entry bindings must name that same root and typed path. The interpreter
and verifier transfer those exact claims together; neither treats aggregate
custody as a Boolean property of the containing parameter.

One affine record argument may therefore carry several disjoint linear sibling
claims. Source checking retains every sibling, terminal production assigns a
dense machine-local claim identity to each one, and calls transfer the complete
canonical set to the callee. A successful bodyless boundary invocation carries
the verifier-derived completion-receipt set for all live claims attached to each
exact argument position, rather than assuming one claim per linear parameter.
Missing, duplicated, reordered, or path-mismatched receipt rows reject before
execution. The interpreter commits their consumption only after the provider
effect succeeds; rejection records no receipt and leaves custody live.

The canonical internal structural-result call form also accepts one whole-root
argument with an exact nonempty finite claim map. Caller transfers, callee entry
claims, every successful callee structural return, returned transfers, and
caller result bindings are one path-preserving bijection. Duplicate, missing,
swapped, overlapping, or path-mismatched rows reject independently in the
codec and verifier. Interpretation transfers the complete map only after the
call charge succeeds, does not replay it across suspension, rebinds it only on
successful return, and leaves crash settlement to the exact live frontier.
Checked-source production remains on its bounded one-claim slice.

The admitted result-bearing slice returns one primitive scalar from a bodyless
boundary while consuming one or more whole structural roots. Its call result,
boundary signature, arguments, and exact receipts survive canonical encoding
and independent verification. Interpretation checks the provider's returned
scalar before committing either custody or receipts, so a rejected call can be
retried against the unchanged frontier. Omega preserves that result in its
abstract plan. An admitted x86-64 `u8` port-read provider lowers to a sealed
instruction interval and returns the byte through the scalar ABI; its provider
identity, whole-root arguments, receipts, and exact bytes survive object,
image, and installation validation. Other result shapes and targets, plus
projected exits and content-bearing returned results, fail closed.

The reference interpreter's host result carrier distinguishes Unit, scalar,
and opaque structural values. A structural boundary result uses the same
target-neutral carrier as a structural entry input: exact structural type,
ordered qualifications, opaque identity, and an empty whole-root path. The
interpreter validates the response before recording the effect or transferring
argument custody, then establishes the result and its affine cleanup frontier.
Suspension after completion does not replay the provider. A handler without
structural-result support rejects before invoking its effect. A malformed host
response leaves interpreter custody unchanged; this does not roll back effects
the host may already have performed. Linear results, result claims, projected
qualifications, and sum discriminator/payload inspection remain unsupported.
This interpreter carrier does not extend native provider installation.
The embedding host remains responsible for returning a legitimate owned value;
the opaque number is not a globally checked allocation identity. In particular,
it cannot establish freshness relative to interpreter-created values or values
retained by suspended callers. Descriptor validation is not proof of external
ownership.

A qualified whole structural parameter whose domain owns a checked `Content<A>`
projection carries that content catalog into terminal Psi on both Unit and
primitive-result bodyless exits. Lowering reuses the structural claim identity
and records the subject at its callable-entry revision plus the owner-unique
projection and algebra; it does not infer content from carrier bytes or from
the domain name.
Vocabulary 27 retains that normalized projection on the owning structural
domain independently from any route, claim, or producer schema. Validation
replays its algebra, expression, carrier paths, and report fingerprint before checking
that every use cites the exact owner definition. A producer therefore cannot
coherently understate capacity by rewriting both its schema expression and its
derived schema identity.
The verifier independently requires the content subject to bind the same exact
entry parameter and claim. Provider rejection therefore preserves the complete
structural/content frontier, while successful completion commits the receipt and
consumes it. This source slice admits only whole parameters. A projected exit
continues to fail closed until an authored partition/residual equation supplies
its geometry.

A stable record claim path may cross nested relevant record fields. Each
segment is resolved against the structural type reached by the preceding
segment, and the complete path remains canonical identity across production,
encoding, direct Unit transfer, interpretation, and boundary settlement. An
unknown inner field rejects, a caller/callee truncation is a custody-set
mismatch, and an ancestor claim cannot coexist with one of its descendant
claims.

The indexed source slice accepts one nonempty literal fixed array of linear
structural elements with the complete dense sibling claim set. One literal
element may pass either to a bodyless Unit boundary or through an ordinary Unit
call whose caller and callee each have exactly one structural parameter and no
scalar parameters. The callee accepts one unqualified whole-root claim and no
contract clause over that parameter. Verification rebases the selected claim;
interpretation retains every sibling until its own successful settlement.
Omega realizes the internal call on all five targets and carries its exact
path, type, layout, copy bytes, and claim transfer through installation.
Nested/dynamic indexes, wider signatures, projected contracts, content-bearing
partitions, partial returns, and aggregate construction remain fenced.

Projected owned transfers share one multiplicity-independent partial-custody
frontier. Once a projection moves, its ancestor cannot be called, returned, or
discarded as a whole; duplicate and ancestor/descendant overlapping moves
reject. The bounded linear fixed-array case closes only when the complete dense
sibling set has transferred, while affine records retain their explicit typed
residual-cleanup route. This verifier rule tracks debt for existing projected
Unit calls; it does not authorize projected `CallStructural` or reconstruction
of a value with a hole.

The native whole-root structural-result lane accepts a direct 8-byte
integer-class placement or a direct 9--16-byte placement split into two
canonical register fragments. System V AMD64 and AAPCS64 realize the latter;
Microsoft x64's indirect aggregate plan remains rejected. Target assignment,
machine emission, object/image validation, and installation replay independently
preserve fragment order, offsets, sizes, and selected registers. Wider or
non-integer-class shapes, multiple roots or claims, projections, staging, and
bodyless calls remain fenced.

Claim-free partial cleanup in checked production accepts one owned, non-self,
unqualified affine parameter whose finite structural graph consists of records
and nonempty literal fixed arrays.
Pairwise prefix-disjoint, nonempty paths may mix record fields and literal
indices through source-ordered one-parameter ordinary Unit calls. Structural
fields carry cleanup; scalar, floating-point, and bounded-owned-byte record
fields retain their existing no-cleanup treatment. Range and arithmetic-policy
constraints on a primitive field do not change that ownership classification;
the source classifier must still reject references to those same carriers.
Claims, content, borrowed
fields, nominal `drop`, cases, dynamic indices, and projected result storage are
not admitted by this route. Indexed transfers retain contract-free callers and
exact ordinary Unit disposers as callees.

The Unit return names every maximal live residual subtree by exact root,
canonical path, and subtree type in recursive reverse declaration/index order.
An untouched array row is one residual, not an expanded list of its leaves.
No partially moved ancestor is discarded whole. When all structural descendants
have transferred, the complement is empty and the existing `ReturnUnit` carries
no cleanup; otherwise the existing `ReturnUnitPartialAffine` carries the exact
complement. No new representation is needed.

Terminal verification, canonical encoding, and interpretation also admit one
claim-free, unqualified affine call-result root. An ordinary whole-value producer
or structural boundary call establishes that exact result place before the
projected ordinary Unit calls. The bounded return schedule has one producer followed
by projected disposers in one block, with no other live roots left at return.
Ordinary production transfers its input into the result; it does not leave a
second owner. The same type-directed complement reconstructs record fields,
array indices, maximal untouched subtrees, and empty remainders. Producer, root
type, path, result metadata, and current live custody rejoin independently.
Reordered production, overlapping moves, whole-root cleanup after a partial move,
and residual type/order drift reject. A crash retains the abandoned residual
frontier and carries no cleanup successor. This does not admit construction-local
partial moves, mixed-root cleanup ordering, or scalar-return residuals.

An ordinary `Jump` can also dispose one such root's exact residual complement
before entering its successor. Its ordered `residual_affine_discards` retain
the place, path, and subtree type; verification reconstructs them from the
actual projected transfers and the live frontier. The root may be an owned
parameter or an ordinary/boundary call result. Unrelated live roots remain live;
mixing whole-root cleanup with residual cleanup on this edge remains unsupported
until one schedule can express their combined establishment order. Indexed
transfers retain the contract-free caller and exact Unit disposer requirements.
Construction-local roots, mixed residual roots, and ranked-backedge cleanup are
not admitted by this route. Existing linear projected custody across ordinary
jumps remains governed by its own claim checks.

Interpretation validates the exact residual transaction, charges the edge,
materializes scalar arguments, disposes the remainder, and then binds successor
parameters. Exhausted fuel leaves every residual live; the next successor effect
observes none of the disposed root's custody. A fully transferred root needs no
residual row. Empty-residual jumps keep their existing tag-1 encoding byte for
byte; nonempty residuals use tag 10 with the same jump semantics. An empty tag-10
payload is noncanonical. Omega's abstract jump and derived control-flow edge
retain the ordered residual rows. Current ownership replay reconstructs the
complement from the actual block's projected transfers and live owner, while
retained Terminal edge-entry/exit snapshots independently bind that transaction.
Unrelated roots remain live and empty complements cannot hide another dying
root. Canonical Omega unit identity includes both operation and derived-edge
rows. Control-flow fusion, threading, and merging reject edges whose residual
cleanup they cannot preserve. Native Unit lowering carries acyclic adjacent
fallthrough jumps as explicit continuation operations. Each retains the real
edge, source and target blocks, and ordered residual schedule. Assignment and
emission replay the live parameter/result roots and exact complement before
retiring that root; unrelated roots remain live. Fixed-integer entry-origin
bindings retain simultaneous Jump assignment and canonical original parameter
identity. Boundary-result projections, computed scalar bindings, and cyclic
cleanup remain outside this native route. Scalar graph
legalization and native publication cannot silently erase those residuals.

Checked-source production carries this result-root schedule for one leading
immutable local initialized by a whole-value ordinary call or structural boundary
call, followed by ordinary Unit disposers. Named field and literal-index operands
rejoin the local's exact producer binding, canonical source path, and captured
projected Transfer. The exit discard retains the local's actual establishment
provenance; a field transfer does not retire the whole root. Residual rows reuse
the structural argument's parameter/result source identity instead of treating a
result ordinal as a parameter index. The ordinary root-only sequencer does not
accept these partial moves. Boundary production retains its declared service
reach; it does not make the subsequent disposers effectful.

An anonymous projected operand such as `Sink::take(Root::forward(value).right)`
retains that partial-return schedule when it is the caller's sole statement.
The producer may be an ordinary whole-value call or a structural boundary call.
The checker establishes the temporary at its exact producer expression,
transfers the selected field/literal-index path, and records the maximal residual
paths with that establishment provenance at the enclosing call continuation.
No named local is synthesized. Captured call ordinals remain preorder while
execution runs the producer before its consumer. The existing return edge owns
cleanup because this continuation ends the caller, preserving that form's
existing native support.

Wider single-state Unit bodies use the ordinary statement sequencer and its
`CallContinuationCleanup` entry. Each projected consumer has one anonymous
ordinary/boundary producer, one owned structural argument, and the existing
empty-body, effect-free Unit disposer. The exact producer expression, statement
coordinate, result binding, selected transfer, and ordered residual permissions
rejoin independently before lowering emits the residual `Jump`. Scalar caller
parameters and earlier scalar locals retain their values through ordinary Jump
bindings; scalar arity, type, and definition checks remain independent of the
result root's residual custody. This admission applies to result-root Jump
continuations, not parameter-root projections or partial final returns. Later
statements and successive producing statements keep their authored order; every
temporary dies before the next statement. Even an empty complement
retains its checked continuation entry, while its Terminal jump needs no discard
rows. Named partial results still use the separate partial-return plan. Multiple
producers or effects within one consumer's argument list, non-Unit consumers,
and mixed dying-root cleanup remain outside this source route. Native lowering
supports ordinary direct-register result producers on acyclic Unit fallthrough
continuations, including successive producers and intervening zero-argument
Unit calls. Fixed-integer entry parameters can cross these jumps and feed later
scalar-only ordinary Unit calls. Register inputs are saved before structural
parameter staging or any call; incoming-stack inputs keep their original ABI
locations. Entry homes reuse the Unit frame, after structural parameter homes
and before result homes. Bodies without continuations retain their existing
storage policy. Object validation reconstructs the entry stores and complete
structural staging prefix; installation evidence retains their exact ordered
locations and byte intervals. Source-to-target validation rejoins authored Jump
bindings and actual consumer origins. Later artifact checks establish retained
binding type, definition, and scope, not independent recovery of which authored
alias a canonicalized consumer used. Computed scalar values and projected
boundary results remain outside this native continuation route.

Lowering and Terminal verification independently reconstruct that complement
from the types and moves, rejecting overlaps, missing or extra residuals, and
path/type/order drift. Reconstruction checks output size before enumerating
array children; a forged huge dimension with a short cleanup list cannot force
an enormous verification scan. Interpretation charges the return edge before
disposing any residual, so fuel exhaustion cannot clean early. Calls retain
authored order regardless of cleanup order.

The governing rule for wider fixed-array carriers is already closed: source
establishes elements in increasing index order, and every ordinary disposing
edge names the exact live residual indices in decreasing order with moved
indices absent. Nested arrays apply that order recursively. Terminal validation
must reconstruct the ordered static sequence from the structural type and
frontier; producer order, runtime liveness flags, and data-dependent cleanup
loops confer no authority. Authored projected moves retain authored order.
Partial construction follows the same rule on its established prefix, while
trap and nuclear-abort terminators carry no cleanup.

Omega ownership and native replay reconstruct the same type-directed finite
record/array complement, including mixed field/index paths, whole-subtree moves,
and empty complements. Native acceptance remains subject to existing structural
layout, size, and ABI limits; Psi acceptance alone does not authorize native
replay. Target assignment, machine emission, object/image validation, and
installation replay retain the exact root and projected types, full-path byte
offset, source and destination homes, authored calls, and reverse residual order.
For an array root, call metadata retains that root's length and element stride;
record-root paths carry neither, even when they pass through an array.
ABI-required indirect argument copies materialize only the transferred subtree.
Cleanup itself emits no instruction, runtime bitmap, or liveness-dependent loop.
Omega's abstract entrance and current ownership replay also carry the ordinary
identity-call result schedule without scalar arguments. The exact producer
transfers its sole owned affine input unchanged, with no claims, qualifications,
contracts, or effects. Replay joins the result declaration to that actual producer
and reconstructs the same complement; an unrelated live result cannot disappear
at Unit return. Structural boundary results use the same producer, type,
availability, and ownership reconstruction while retaining their boundary
signature and declared service reach. Result custody is installed only after
the call's input transfers and completion receipts; it grants no input claim
completion. Native production also admits one leading ordinary identity result
followed by projected Unit disposers, with no other live roots. Its claim-free,
unqualified record or fixed-array result retains the direct structural-return
ABI limits below. Each direct-register fragment retains its exact one-to-eight
byte extent. Fragments of three, five, six, or seven bytes use byte accesses and
shifts rather than accessing alignment padding; the existing one-, two-, four-,
and eight-byte encodings stay unchanged. The same packing serves incoming
parameter staging, outgoing argument copies, and result stores. R10 on x86-64
and X16 on AArch64 are caller-clobbered scratch registers, disjoint from the
native argument/result registers and indirect source bases. Stores preserve
their source register; loads preserve an indirect base. Aggregate result homes
align to at least eight bytes while retaining the exact logical byte extent; alignment
padding precedes the home and cannot authorize an oversized store. The call
stores each fragment at its actual width into a distinct result home before
later projected copies; the result is not relabeled as an input parameter or a
tagged sum. Assignment, object validation, and installation replay retain the real producer/result,
independently reconstruct the store bytes and intervals, reject overlapping
homes, and reconstruct the residual or empty complement. The single-final-call
anonymous source form uses these same result homes and projected copies.
The acyclic Unit continuation route reuses that storage across multiple owned
parameters and successive producers. Cleanup has a separate zero-byte
continuation record even when its complement is empty; the final return record
contains only owners still live there. Object and installed-artifact replay
reconstruct the call/edge sequence, projected layouts, and exact residual
partition, binding each boundary to its real operation ordinal and code offset.
Continuation block endpoints remain retained emission metadata: downstream
replay checks their adjacent acyclic chain but does not infer an independent
authored block map from machine bytes. Canonical installation retains the
continuation list separately from final-return cleanup.
Boundary-result native projection, claims, nominal
destruction, and partial construction remain outside
this native slice.

Free and static attached Unit machines use the same checked result-root cleanup
plan, including ordinary and boundary result producers. Signature selection
rejoins the optional attachment exactly; no attachment means no implicit `self`
or invented carrier. Free structural disposers use the ordinary typed parameter
and whole-root cleanup rules. A static attachment supplies no implicit receiver
argument. The native boundary-result restriction above still applies.

The straight-line Unit return slice carries explicit no-code cleanup for owned
affine structural parameters that have no claim rows. The checked plan derives
the list from state-exit permission events in reverse parameter declaration
order. Terminal verification independently reconstructs the exact live affine
frontier, and rejects missing, extra, reordered, unknown, or claim-bearing
discards. Interpretation charges the return edge before removing those places,
so sponsor exhaustion cannot perform cleanup early. A one-state Unit/effect
body may also begin with a finite source-ordered run of immutable, unqualified,
empty-record affine locals. Each has an explicit fuel-charged establishment;
the return discards locals in reverse order before eligible parameters. Their
typed custody crosses Omega's five native artifact pipelines without runtime
bytes. Nonempty, mutable, qualified, content-bearing, nominal-cleanup, and
post-effect locals remain fenced.

The bounded construction-prefix extension admits exactly one additional local
shape: an uninitialized mutable `[T; 3]` with empty, unqualified, claim-free
affine-record elements, followed by literal establishments of indices `0` then
`1` and an ordinary Unit return. Terminal publishes two zero-ABI local places
whose construction metadata retains the common array-root type and indices.
The verifier reconstructs the exact root declaration, increasing establishment
sequence, decreasing cleanup sequence `[1, 0]`, and static operation/edge fuel
ordinals. The codec, Omega lowering, native emission, object/image replay, and
installation encoding retain the same metadata; none may infer it from layout
or introduce a runtime liveness bitmap. Dynamic indices and wider construction
plans remain fenced.

The exact next construction-prefix carrier admits the same shape at length
four with establishments `[0, 1, 2]`. Terminal publishes three ordered
zero-ABI local places, the Unit return discards them as `[2, 1, 0]`, and the
verifier, codec, interpreter, Omega lowering, native emission, object/image
replay, and installation encoding retain the common root plus exact four fuel
units. Missing/reordered operations or cleanup, changed indices/root length,
and wider prefixes at that rung reject; no runtime liveness bitmap or cleanup
loop is introduced.

The following bounded carrier admits the same shape at length five with
establishments `[0, 1, 2, 3]`. Terminal publishes four ordered zero-ABI local
places, the Unit return discards them as `[3, 2, 1, 0]`, and the verifier,
codec, interpreter, Omega lowering, native emission, object/image replay, and
installation encoding retain the common root plus exact five fuel units.
Missing/reordered operations or cleanup, changed indices/root length, and wider
prefixes reject; no runtime liveness bitmap or cleanup loop is introduced.

The next bounded carrier admits the same shape at length six with
establishments `[0, 1, 2, 3, 4]`. Terminal publishes five ordered zero-ABI local
places, the Unit return discards them as `[4, 3, 2, 1, 0]`, and the verifier,
codec, interpreter, Omega lowering, native emission, object/image replay, and
installation encoding retain the common root plus exact six fuel units.
Missing/reordered operations or cleanup, changed indices/root length, and
wider prefixes at that rung reject; no runtime liveness bitmap or cleanup loop
is introduced.

The following bounded carrier admits the same shape at length seven with
establishments `[0, 1, 2, 3, 4, 5]`. Terminal publishes six ordered zero-ABI
local places, the Unit return discards them as `[5, 4, 3, 2, 1, 0]`, and the
verifier, codec, interpreter, Omega lowering, native emission, object/image
replay, and installation encoding retain the common root plus exact seven fuel
units. Missing/reordered operations or cleanup, changed indices/root length,
and other prefix drift reject; no runtime liveness bitmap or cleanup loop is
introduced.

The next bounded carrier admits the same shape at length eight with
establishments `[0, 1, 2, 3, 4, 5, 6]`. Terminal publishes seven ordered
zero-ABI local places, the Unit return discards them as `[6, 5, 4, 3, 2, 1, 0]`,
and the verifier, codec, interpreter, Omega lowering, native emission,
object/image replay, and installation encoding retain the common root plus
exact eight fuel units. Missing/reordered operations or cleanup, changed
indices/root length, and other prefix drift reject; no runtime liveness bitmap
or cleanup loop is introduced.

The following bounded carrier admits the same shape at length nine with
establishments `[0, 1, 2, 3, 4, 5, 6, 7]`. Terminal publishes eight ordered
zero-ABI local places, the Unit return discards them as
`[7, 6, 5, 4, 3, 2, 1, 0]`, and the verifier, codec, interpreter, Omega
lowering, native emission, object/image replay, and installation encoding
retain the common root plus exact nine fuel units. Missing/reordered operations
or cleanup, changed indices/root length, and other prefix drift reject; no
runtime liveness bitmap or cleanup loop is introduced.

The next bounded carrier admits the same shape at length ten with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8]`. Terminal publishes nine ordered
zero-ABI local places, the Unit return discards them as
`[8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the verifier, codec, interpreter, Omega
lowering, native emission, object/image replay, and installation encoding
retain the common root plus exact ten fuel units. Missing/reordered operations
or cleanup, changed indices/root length, and other prefix drift reject; no
runtime liveness bitmap or cleanup loop is introduced.

The following bounded carrier admits the same shape at length eleven with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]`. Terminal publishes ten ordered
zero-ABI local places, the Unit return discards them as
`[9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the verifier, codec, interpreter, Omega
lowering, native emission, object/image replay, and installation encoding
retain the common root plus exact eleven fuel units. Missing/reordered
operations or cleanup, changed indices/root length, and other prefix drift
reject; no runtime liveness bitmap or cleanup loop is introduced.

The next bounded carrier admits the same shape at length twelve with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]`. Terminal publishes eleven
ordered zero-ABI local places, the Unit return discards them as
`[10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the verifier, codec, interpreter,
Omega lowering, native emission, object/image replay, and installation encoding
retain the common root plus exact twelve fuel units. Missing/reordered
operations or cleanup, changed indices/root length, and other prefix drift
reject; no runtime liveness bitmap or cleanup loop is introduced.

The following bounded carrier admits the same shape at length thirteen with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]`. Terminal publishes
twelve ordered zero-ABI local places, the Unit return discards them as
`[11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the verifier, codec, interpreter,
Omega lowering, native emission, object/image replay, and installation encoding
retain the common root plus exact thirteen fuel units. Missing/reordered
operations or cleanup and changed indices/root length reject; no runtime
liveness bitmap or cleanup loop is introduced.

The next bounded carrier admits the same shape at length fourteen with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]`. Terminal publishes
thirteen ordered zero-ABI local places, the Unit return discards them as
`[12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the verifier, codec,
interpreter, Omega lowering, native emission, object/image replay, and
installation encoding retain the common root plus exact fourteen fuel units.
Missing/reordered operations or cleanup, changed indices/root length, and
other prefix drift rejects; no runtime liveness bitmap or cleanup
loop is introduced.

The following bounded carrier admits the same shape at length fifteen with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]`. Terminal
publishes fourteen ordered zero-ABI local places, the Unit return discards them
as `[13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the verifier, codec,
interpreter, Omega lowering, native emission, object/image replay, and
installation encoding retain the common root plus exact fifteen fuel units.
Missing/reordered operations or cleanup, changed indices/root length, and
other prefix drift rejects; no runtime liveness bitmap or cleanup
loop is introduced.

The next bounded carrier admits the same shape at length sixteen with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]`. Terminal
publishes fifteen ordered zero-ABI local places, the Unit return discards them
as `[14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the verifier,
codec, interpreter, Omega lowering, native emission, object/image replay, and
installation encoding retain the common root plus exact sixteen fuel units.
Missing/reordered operations or cleanup, changed indices/root length, and
other prefix drift rejects; no runtime liveness bitmap or cleanup loop is
introduced.

The following bounded carrier admits the same shape at length seventeen with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]`.
Terminal publishes sixteen ordered zero-ABI local places, the Unit return
discards them as `[15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and
the verifier, codec, interpreter, Omega lowering, native emission, object/image
replay, and installation encoding retain the common root plus exact seventeen
fuel units. Missing/reordered operations or cleanup, changed indices/root
length, and other prefix drift reject; no runtime liveness bitmap or cleanup
loop is introduced.

The next bounded carrier admits the same shape at length eighteen with
establishments `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]`.
Terminal publishes seventeen ordered zero-ABI local places, the Unit return
discards them as
`[16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the
verifier, codec, interpreter, Omega lowering, native emission, object/image
replay, and installation encoding retain the common root plus exact eighteen
fuel units. Missing/reordered operations or cleanup, changed indices/root
length, and other prefix drift reject; no runtime liveness bitmap or cleanup
loop is introduced.

The following bounded carrier admits the same shape at length nineteen with
establishments
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]`. Terminal
publishes eighteen ordered zero-ABI local places, the Unit return discards them
as `[17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and the
verifier, codec, interpreter, Omega lowering, native emission, object/image
replay, and installation encoding retain the common root plus exact nineteen
fuel units. Missing/reordered operations or cleanup, changed indices/root
length, and other prefix drift reject; no runtime liveness bitmap or cleanup
loop is introduced.

The next bounded carrier admits the same shape at length twenty with
establishments
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18]`. Terminal
publishes nineteen ordered zero-ABI local places, the Unit return discards them
as `[18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and
the verifier, codec, interpreter, Omega lowering, native emission, object/image
replay, and installation encoding retain the common root plus exact twenty fuel
units. Missing/reordered operations or cleanup, changed indices/root length,
and other prefix drift reject; no runtime liveness bitmap or cleanup loop is
introduced.

The following bounded carrier admits the same shape at length twenty-one with
establishments
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]`.
Terminal publishes twenty ordered zero-ABI local places, the Unit return
discards them as
`[19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`, and
the verifier, codec, interpreter, Omega lowering, native emission, object/image
replay, and installation encoding retain the common root plus exact twenty-one
fuel units. Missing/reordered operations or cleanup, changed indices/root
length, and other prefix drift reject; no runtime liveness bitmap or cleanup
loop is introduced.

The next bounded carrier admits the same shape at length twenty-two with
establishments
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]`.
Terminal publishes twenty-one ordered zero-ABI local places, the Unit return
discards them as
`[20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`,
and the verifier, codec, interpreter, Omega lowering, native emission,
object/image replay, and installation encoding retain the common root plus
exact twenty-two fuel units. Missing/reordered operations or cleanup, changed
indices/root length, and other prefix drift reject; no runtime liveness bitmap
or cleanup loop is introduced.

The following bounded carrier admits the same shape at length twenty-three
with establishments
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21]`.
Terminal publishes twenty-two ordered zero-ABI local places, the Unit return
discards them as
`[21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`,
and the verifier, codec, interpreter, Omega lowering, native emission,
object/image replay, and installation encoding retain the common root plus
exact twenty-three fuel units. Missing/reordered operations or cleanup, changed
indices/root length, and other prefix drift reject; no runtime liveness bitmap
or cleanup loop is introduced.

The next bounded carrier admits the same shape at length twenty-four with
establishments
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22]`.
Terminal publishes twenty-three ordered zero-ABI local places, the Unit return
discards them as
`[22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`,
and the verifier, codec, interpreter, Omega lowering, native emission,
object/image replay, and installation encoding retain the common root plus
exact twenty-four fuel units. Missing/reordered operations or cleanup, changed
indices/root length, and other prefix drift reject; no runtime
liveness bitmap or cleanup loop is introduced.

The following bounded carrier admits the same shape at length twenty-five with
establishments
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]`.
Terminal publishes twenty-four ordered zero-ABI local places, the Unit return
discards them as
`[23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`,
and the verifier, codec, interpreter, Omega lowering, native emission,
object/image replay, and installation encoding retain the common root plus
exact twenty-five fuel units. Missing/reordered operations or cleanup, changed
indices/root length, and other prefix drift reject; no runtime
liveness bitmap or cleanup loop is introduced.

The next bounded carrier admits the same shape at length twenty-six with
establishments
`[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]`.
Terminal publishes twenty-five ordered zero-ABI local places, the Unit return
discards them as
`[24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`,
and the verifier, codec, interpreter, Omega lowering, native emission,
object/image replay, and installation encoding retain the common root plus
exact twenty-six fuel units. Missing/reordered operations or cleanup, changed
indices/root length, and length-twenty-seven or wider prefixes reject; no runtime
liveness bitmap or cleanup loop is introduced.

The nominal-cleanup slice accepts one root-only, one-state Unit machine with a
finite nonempty list of claim-free, unqualified affine parameters whose records
are empty or contain only relevant Terminal-supported Boolean/integer fields, plus
their exact attached `T::drop(&mut self)` machines. One cleanup may be empty or
contain a finite source-ordered list of ordinary zero-argument calls to mutually
distinct exact-empty attached helpers. Multiple cleanups run in reverse parameter
declaration order; every body may use that executable form, including a shared
cleanup target or helper. Repeated use of the same cleanup machine remains legal
because each action names a distinct place. The
return carries the ordered whole-place/type/machine list. Verification
reconstructs its exact deduplicated machine closure; interpretation charges the
caller edge once and executes each cleanup sequentially; fixed fuel counts
every invocation, including a repeated target. Omega carries all return
cleanup kinds in one ordered action stream through abstract, target, assigned,
machine, object, image, and installation artifacts. Empty drops emit no native
call; an executable body emits a call owned by the exact edge/action ordinal
before teardown, with source-ordered operation-owned helper calls.
For an empty cleanup body or the bounded receiver-independent helper-call body,
a finite canonical set of direct Boolean-field clauses in either polarity is
accepted when the caller's supported
Boolean fact set contains every corresponding requirement on each owned root.
Caller-only facts remain in the entry contract. Terminal Psi retains a
target-local proof-only receiver, shared by actions using the same target, and
one positional edge obligation per action and cleanup clause. It substitutes
that receiver with the owned cleanup place during
independent verification, and binds each proof to its matching caller
assumption rather than assuming identical set positions. The cleanup target
remains operationally zero-argument. The verified Psi-to-Omega boundary
removes those proof-site identities; every downstream Omega validator rejects
their reintroduction. Missing caller evidence rejects during checked
production. Wider predicates, bodies that can inspect or change receiver facts,
nested/erased receivers, claims, qualifications, locals, and non-root edges
remain fenced.

The exact attached cleanup machine is a compiler-only edge dependency. Source
cannot select it as a call, static-machine value, or forwarded declaration;
authored early disposal is the ordinary consuming `omega::core::drop(value)`
call, whose callee edge receives the same checked plan. The complete row keeps
Type-side discharge eligibility separate from proposition prerequisites,
operational reach/effects/work, and derived guarantees. An unmet proposition
premise must be proved locally or already be authored in `requires`; cleanup
analysis never promotes it into a new caller demand.

The eventual erased-owner carrier retains payload/storage custody beside size,
alignment, movement, and this exact cleanup plan in its compiler-built
descriptor. Terminal verification must reconstruct that the descriptor plan
belongs to the hidden concrete type and remains eligible under the package's
retained facts. Borrowed erased views carry no referent-cleanup disposition.
This is descriptor lifecycle metadata, not a `Drop` conformance.

An unconditional jump and each ordered conditional successor may carry an
independent canonical reverse-declaration subset of the same eligible
parameters. Verification removes exactly those places from the corresponding
successor frontier; interpretation charges the selected edge and materializes
its scalar arguments before committing the no-code disposal. The primitive-only
scalar source producer emits canonical empty lists. Checked facts now retain an
exact source-state, transition-statement, and target-state row for each
supported structural jump or conditional arm, together with the
reverse-declaration positions of its claim-free affine parameter discards.
States needing affine-local, projected, nominal, or claim-bearing cleanup fail
closed without a partial row. The first structural-control producer consumes a
composed checked plan for attached, multi-state, Unit-returning machines whose
states contain only claim-free affine structural parameters and either return
naturally, contain one unconditional ordinary local jump, or select two ordered
ordinary successors from one retained Boolean scalar input. At most two states
may select successors, so an unconditional prefix and one nested decision are
accepted while a third conditional state remains fenced. Whole-parameter
arguments provide exact type-preserving transfer maps; each map and its exact
cleanup row must independently partition the source frontier. Production
resolves checked parameter positions against the source-handle-free state
signatures. One acyclic two-predecessor join may reconverge when both paths
reconstruct the identical ordered structural frontier. Scalar arguments remain
ordinary typed edge bindings and need not be the same values. A divergent
custody map, second join, or third predecessor rejects.

Effectful multi-state Unit control uses a separate additive checked carrier;
it does not weaken the claim-free structural-control carrier above or attach
operations to it. The first exact composed family has three acyclic states: an
operation-free entry with either one Boolean scalar parameter or one exact
closed Boolean guard and two ordered successors, plus two parameter-free leaves
containing exactly one scalar-only bodyless boundary call followed by Unit
return. Closed comparisons whose const substitution leaves only anonymous
integer literals are evaluated mathematically at the checked boundary and
retained as Boolean constants; no downstream stage guesses an integer width.
The carrier retains per-state
operations, machine-wide contract and service reach, and exact boundary call
coordinates. Typed-to-checked construction removes the whole machine if either
leaf or boundary catalog entry is unavailable. Checked-to-Terminal lowering
rejoins the original flow call and contract identities, publishes the selected
attachment, boundary, and service catalogs, and emits one conditional block
and two effectful return blocks. One implicit borrowed `self` may remain
attachment context rather than a runtime structural parameter. When that
attachment has exactly one provider-backed field and both leaves call through
it, shared provider admission requires a complete canonical requirement set
and emits exact `ProviderAttachment` roots. Ambiguous provider fields,
structural arguments, claims, internal calls, and larger graphs remain fenced. The lowering
entrance only coordinates named `admission`, `catalogs`, and `emission`
submodules; its typed producer is likewise a small coordinator over named
`topology`, `guards`, `leaves`, and `assembly` submodules.

Unranked cycles admit scalar computations and block parameters, immutable shared
byte views, and the supported Unit-effect operations. Full-graph dominance and
exact successor bindings apply to scalar values and view descriptors, including
multi-entry cycles. Owned or mutable custody and wider operations remain fenced.
Ownership replay visits every reachable block and edge,
including exits downstream of a cycle. A first incoming frontier seeds each
block; deterministic transfer and exact comparison of every later arrival
establish the ownership fixed point. No cleanup, return, or crash-frontier check
may disappear because an acyclic ordering has a cyclic remainder. This does not
establish a termination guarantee.

Proof reconstruction likewise retains every normal return, including returns
downstream of a cycle. A deterministic DFS cuts ancestor edges only for proof
scheduling; executable edges remain intact. Every cut target starts with no
incoming semantic axioms. Operations and selected guards then establish facts
on the remaining acyclic paths, and all normal returns participate in the exit
intersection. A fact from the acyclic prefix or one loop iteration is not an
invariant. An infinite component contributes no normal return and does not
itself require a termination proof. General invariant reconstruction and ranking
relations beyond the natural-rank slice below remain unfinished; guarded-crash
path enumeration also remains restricted on cyclic graphs. Omitting an exit
from a published `ensures` check is never a substitute for that work.

The legacy `TerminalRankedScc::UnsignedCountdown` representation retains its
separate interpreter, fixed-fuel, and native execution routes. Its acyclic
skeleton establishes the header frontier, one complete covered cycle computes
the preservation candidate, and all live claims, owned places, and partial-custody
paths must match exactly before representation
admission. The reference interpreter has a distinct validation and verification
carrier for only the one-machine structural Unit countdown: its proof scheduler
removes the already validated covered backedge, reconstructs the taken
`0 < remaining` edge as the discrete unsigned `1 <= remaining` subtraction
premise, and requires the exact-subtract evidence before constructing resumable
  execution state. Ordinary verification continues to reject this legacy variant,
  so fixed-fuel and Omega/native consumers cannot acquire authority through the
  interpreter path; provider installation and extra mixed work remain fenced.
  Fixed-fuel and native lowering instead use separate opaque verifier carriers
  for this exact machine. Their lower projection is replayable data: canonical
  Terminal semantics and proof bytes, exact fixed-fuel fields, the two relevant
  complete converged frontier snapshots, and the reconstructed
  preheader/header/guard/subtract/backedge graph. The object boundary decodes
  and checks the proof again under native and fixed-fuel admission, re-derives
  the certificate, and compares every retained frontier row. Representation
  crates therefore do not import semantic-layer authority merely to make an
  in-memory producer result unforgeable. Target lowering replays the exact
  graph, affine-owned structural parameter or persistent mutable receiver,
  exit cleanup, and ABI placement. A receiver retains the reference's usage
  multiplicity, an empty owned cleanup frontier, and the existing
  `BorrowedReference` shape with the referent's size/alignment rather than an
  integer-tagged pointer. Assignment and physical replay independently derive
  its referent shape and native pointer placement from retained declarations;
  coordinated value-shape or placement substitutions reject.
  Checked-source export retains fixed and nested primitive arrays inside the
  receiver, including exact lengths and integer, Boolean, or IEEE float element
  types. Primitive leaves remain scalar declarations, not synthetic records;
  array-bearing receivers use the same reference ABI and empty owned frontier.
  Assignment accepts only the target-prescribed rank register. The ordinary selected-instruction
  path stays closed, while a disjoint unoptimized route emits the exact Linux
  x86-64 and AArch64 countdown bodies from assigned custody. The machine-code
  carrier retains the semantic custody, complete ABI/structural inputs, and
  canonical four-operation/five-edge logical-fuel attribution. Ordinary object
  replay independently decodes each target body and reconstructs the exact
  Psi/entry, rank, fixed-fuel, ABI, structural-frontier, cleanup, provenance,
  and nine-row attribution contract. It also rejoins the projected graph,
  structural signature, and type declarations to the verifier-owned semantic
  module, rejecting coherent coordinate substitution rather than accepting a
  merely self-consistent record. Its object function retains the complete
  ranked record, and a stripped record remains a hard failure. Ranked-aware
  semantic-code attribution records the exact operation/edge-to-byte
  correspondence without changing the emitted control flow. It is provenance
  for replay and analysis, never authority to insert runtime accounting.

The current native ranked admission route selects the entry machine and requires
its ranked component and exact countdown ceiling. It does not admit an ordinary
caller invoking a ranked callee. The settled
[projected-receiver call model](../../design_briefs/termination_ranking_and_progress.md#ranked-callees-on-projected-receivers)
establishes a fixed field borrow at the call and preserves that same receiver
through the callee's backedges; no receiver-rebinding syntax or second parent
parameter is needed. Complete support must compose argument references,
call/return and cleanup, callee ranking validation, and resource accounting.
The one-structural-parameter replay restriction belongs to the bounded countdown
implementation, not to general ranked-loop semantics. Widening it alone would
not implement changing-reference transfers or their borrow/ranking checks.

General cyclic control is nevertheless part of Terminal Psi's durable semantic
model rather than a second loop language. `Jump` and `Conditional` form the
graph; block parameters and exact successor arguments carry SSA values around a
cycle. No `Loop` terminator, implicit induction variable, or optimizer-owned
progress rule is introduced. Ordinary verification and reference interpretation
now accept `TerminalRankedScc::Natural` over the supported scalar and immutable
byte-view cyclic graph. The unsigned `n > 0` / `n - 1` countdown above remains
the only ranked native and fixed-fuel implementation slice;
`NonExecutableRankedScc` is an implementation fence for unsupported consumer
routes, not the final language invariant.

For every finite cyclic component the verifier derives canonical SCC topology
from the actual graph: members, entries, exits, and internal edges. A
`CycleComponentId` combines the owning machine identity with that canonical
edge membership. Producer material may cite the component but cannot assert its
topology. The verifier computes the ownership-frontier fixed point and requires
every incoming edge to supply the exact scalar parameter telescope; scalar
values may differ through ordinary block parameters. The structural frontier
must agree exactly on places, claims, partial custody, and cleanup debt.
Backedges perform only their authored transfers and discards; cleanup runs on a
real exit or explicit discard, never merely because an edge closes a cycle.
Reducibility remains an optimizer classification rather than an
execution-legality rule.

Progress evidence is separate from cyclic safety. A productive transition SCC
may remain unranked and run indefinitely. A machine publishing termination
instead carries a producer-supplied, verifier-checked certificate that binds a
well-founded relation and the rank at the relevant blocks to every derived
in-component edge. Source measures such as slice length, bounded distance,
lexicographic order, and declared measures normalize into that relation-plus-
decrease form; Terminal does not grow one operation variant per source spelling,
and the verifier checks the cited proof rather than searching for a measure.
Optimizer rewrites that change component membership, carried state, or decrease
edges invalidate the old certificate and must pass ordinary Terminal
verification again.

The current natural-rank carrier retains one `TerminalNaturalCycle` per complete
cyclic component, with canonical block ranks and every internal edge. Each
component selects one fixed unsigned integer carrier. A block rank is a scalar
machine/block parameter or an actual `ByteSequenceLength` observation. The
verifier independently derives the full SCC roster and exact edge membership,
checks rank type and dominance, and substitutes the target rank through the
selected scalar or structural successor arguments. A dominating length value
cannot be reused after its descriptor has been rebound without reexecuting the
observation. General projections and other ranking views remain unsupported.

Each internal edge requires a proof of `successor_rank <= source_rank` for
preservation or `successor_rank < source_rank` for strict descent. Preserving
implementation-staging edges are allowed only when strict edges meet every
cycle: removing the strict edges must leave an acyclic graph. A preserving
cross-edge cycle therefore rejects even if a DFS-discovered cycle decreases.
This staging rule does not relax the authored state-transition decrease rule.

`proof_bundle.control_cycles` retains one grouped certificate per reconstructed
component, separate from proof-only call recursion. The generic proof-admission
checker checks one shared well-foundedness citation for the fixed unsigned
natural order and every internal edge's comparison against verifier-derived
facts. Missing, surplus, reordered, or substituted evidence rejects. Rank
bindings and comparison choices enter the proof-question commitment separately
from topology identity. The ordinary verified synopsis reports the component,
shared citation, and preserving and strict edge evidence.

Topology, substitution, and proof-question reconstruction belong to
`terminal-verifier/src/control_cycles`. The shared source producer retains
the authored `Slice::Length` witness in
`checked-trees-to-lowered-psi/src/attached_unit/composed_control/state_graph/ranking.rs`;
`control_cycle_proofs.rs` produces evidence for the verifier's reconstructed
questions. No synthetic countdown replaces the actual view extent. This slice
keeps mutable/owned cyclic custody, general projections, wider ranking views,
and callee-progress composition outside its authority. Natural ranking alone
grants neither native byte-view realization nor a quantitative fixed-fuel bound.

Proof-only call recursion uses the same one-certificate-per-component rule but
is not an executable control-flow SCC. `TerminalModule` retains one canonical
row per reachable proof component: a semantic component key is reconstructed
from its rank relation, closed finite-inductive proof-type graph, exact member
contracts and rank parameters, and every exact statement, expression, or
transition call coordinate. A strict structural-subterm path is accepted only
when the verifier resolves every field in that proof-type graph and the
nonempty path returns to the component rank type. These proof types grant no
runtime layout or projection authority.

The proof bundle carries a separately grouped certificate keyed by the
reconstructed component identity. The verifier reconstructs one semantic
ranking-relation identity, one shared well-foundedness obligation, and one
decrease obligation per exact call row, invokes the generic proof-admission
checker, and retains the resulting component acceptance separately from flat
contract evidence. Missing, surplus, reordered, or substituted component or
edge evidence rejects. Terminal format 59 / vocabulary 62 and proof format 22
bind these rows; the ordinary verified synopsis reports the component, shared
well-foundedness route, members, and every exact decrease route. The checked-
to-Terminal producer walks the selected root's contract and body proof-call
closure, resolves machine and entry-state targets to the owning machine, and
retains only complete reachable components. It independently rejoins every
member and edge to the checked rank parameters and common rank type before
erasing frontend handles, then replaces every surviving declaration with a
managed-package or toolchain hermetic identity. It derives the grouped proof
routes only after the verifier has reconstructed the exact component
obligations; the producer never supplies the semantic component, relation, or
obligation identities.

Accepting this certificate establishes only the stated Terminal semantics. An
untrusted or modified producer may emit a different safe cyclic module, just as
it may emit different acyclic code. Binding the module to owner-approved Omega
source requires the separate source-to-Psi correspondence edge; native
execution additionally requires the Psi-to-target refinement edge. No producer
identity, including the canonical Omega compiler's identity, substitutes for
either proof.

Unconditional jumps and conditional arms may additionally pass
direct primitive scalar inputs into typed successor block parameters; the edge
materializes those arguments before committing its structural cleanup.
Production emits the resulting jump/conditional/return blocks and rejects stale
scalar or structural signatures, arm order, or cleanup. Apart from the exact
ranked-countdown preservation rule above, this slice admits only reachable,
acyclic custody lineages whose surviving place order remains canonical. Wider
joins, unranked or differently shaped cycles, reordering, computed guards or
successor values, locals, and richer cleanup continue to fail closed. The
terminal verifier remains responsible for
reconstructing every emitted cleanup frontier and scalar edge binding.

The first nonempty scalar-return source path composes the same cleanup evidence
with an attached, one-state signature containing only claim-free affine
structural parameters. Its scalar work is an ordered prefix of immutable
primitive locals followed by one return expression. Every initializer and the
return use checked scalar expressions: explicitly landed integer literals,
terminal integer operations and casts, Boolean constants, negation, equality,
comparisons, and references to already materialized scalar locals. Initializers
are branch-free except for the repeated Boolean continuations below. State
parameters are partitioned explicitly: primitive inputs receive dense scalar
positions plus retained authored positions, while affine custody retains its
separate structural positions. Their authored-position maps must be disjoint and
complete. Locals follow the scalar inputs in the value namespace. The checked
row carries that partition, the exact structural signature, local types and
statement coordinates, scalar result carrier, return coordinate, and
reverse-declaration cleanup positions. Production revalidates the partition and
the dense scalar/local namespace, materializes expressions in order,
reconstructs any exact-operation proofs before the return edge, and resolves
cleanup positions to structural places. A final short-circuit Boolean return is
expanded into explicit decision blocks: internal conditional edges preserve the
unchanged structural frontier, and every terminal value leaf carries the same
checked complete cleanup list. The verifier reconstructs that requirement on
each path. Any finite sequence of short-circuit Boolean locals is also accepted
within an otherwise branch-free primitive binding sequence: prefix values
dominate the first decision tree, each tree's leaves jump without cleanup to one
typed Boolean convergence parameter, and branch-free work in that continuation
may lead to the next local tree or to the return expression. That final return
may itself be a short-circuit Boolean tree; every one of its value leaves then
performs the same complete cleanup.
Calls, mutable or non-scalar locals, contracts beyond the bounded premises
described below, claims, effects, and multi-state control remain outside this
source slice;
structural custody is never represented as a scalar parameter.
One narrower nominal branch admits a finite nonempty list of direct affine
structural parameters that may mix no-code and nominal roots, a finite set of
direct primitive scalar inputs interleaved at authored parameter positions,
and no authored contract beyond a combination of the direct-Boolean contextual
subset and direct unsigned scalar-parameter upper bounds described below,
plus a finite source-ordered prefix of immutable branch-free primitive locals
and either one branch-free scalar result or a finite Boolean continuation chain
that begins with a finite `&&`/`||` decision tree of arbitrary nesting. Every
later local in that chain is branch-free or another finite nested decision
tree over the inputs and available locals, and it uses its immediate Boolean
predecessor at least once; the return directly names the final local. Checked
plans retain the complete authored parameter partition; terminal Psi gives scalar
values and structural places independent dense namespaces. Terminal production
materializes the input-dependent local and result operations in
source order, then executes the complete cleanup stream in reverse authored
root order. No-code roots retain their exact position without invoking a
machine; nominal targets may be distinct or shared, and each drop may be empty
or contain the bounded source-ordered zero-argument helper-call body accepted
by the Unit nominal slice. For the finite Boolean form, terminal production
retains a branch-only decision tree with distinct return edges and attaches the
same complete cleanup stream to every leaf. Terminal production retains the
cleanup targets and helpers in the same closed module. Contextual cleanup requirements are accepted
for a finite mixed root list in the same direct-Boolean subset as Unit cleanup.
Checked production binds every target premise to the exact nominal caller root
and retains supported caller-only facts on no-code roots; terminal Psi carries
canonical caller requirements, proof-only receivers, and distinct action
obligations. Omega consumes those facts only after verification and projects
the proof metadata away before target lowering. Native lowering preserves the
computed ABI result and, on AArch64, the return link across executable cleanup
calls in an exact lifetime frame; object construction validates the frame,
stores, loads, calls, and stack ceiling from emitted bytes. The finite Boolean
form instead retains one edge-specific cleanup interval per surviving native
leaf and validates the result and return-link lifetime independently on every
native path. Terminal production decides every short-circuit local once per
stage, substitutes each resulting value leaf into the continuation, and
source-distributes branch-free work and later decision stages without a
convergence block. One bounded exception accepts a finite `!`/`&&`/`||` tree
over a finite nonempty set of runtime Boolean parameters and constants. Boolean
equality with a constant normalizes to the same identity/negation leaves. Every
typed value leaf jumps to one terminal-Psi Boolean parameter and one shared
cleanup return. Omega retains the source-ordered decisions, an unconditional
join branch from every non-final leaf, and final-leaf fallthrough into one
physical cleanup tail on every target; object construction reconstructs the
decision regions, decodes every join, and replays the shared tail before image
and installation custody. That exception also admits one canonical direct
relevant Boolean field identity from one claim-free affine nominal-cleanup root,
combined with those parameters and constants. At least one Boolean parameter
must remain in the tree so native expression scratch cannot overwrite the
structural source. Terminal Psi names the exact source place and field ID;
verification reconstructs that field from the entry type, and interpretation/
native lowering read the exact structural ABI field without treating opaque
identity as layout. Machine-code evidence binds every such read to its exact
native interval. Object replay independently resolves the claimed source home,
reconstructs the canonical Boolean field and ABI offset from the retained
structural type closure, replays the live stack depth, and requires the
architecture-specific load and normalization bytes before image and
installation custody. Separately, direct integer comparisons whose
operands are scalar parameters or landed constants, optionally beneath up to
two total bitwise-not, binary bitwise, wrapping shift/arithmetic, saturating
arithmetic, or integer-widening shells, or one exact fixed-width narrowing,
same- or cross-sign, under retained direct parameter range `requires`, or exact
fixed-width addition with a landed operand, subtraction with a landed
subtrahend, or multiplication with a landed factor under retained matching
direct parameter bounds, runtime unsigned multiplication under retained
`1 <= right` and `left <= MAX / right` bounds, or runtime signed multiplication
under a retained positive or at-most-`-2` factor bound and both carrier-tight
quotient bounds, one runtime unsigned addition under the retained canonical
`left <= MAX - right` bound, one runtime signed addition under matching sign and
`MAX - right` or `MIN - right` bounds, runtime subtraction unsigned under a
retained direct subtrahend-to-minuend bound or signed under matching sign and
`MIN + right` or `MAX + right` bounds, one exact
right shift under a direct upper count bound for unsigned counts or direct
zero-lower and upper bounds for signed counts, one exact left shift by a landed
count or runtime count under the verifier-required direct value bounds and, for
runtime counts, direct count upper bounds plus a direct zero lower bound when
that count is signed, or exact division/remainder by a landed nonzero unsigned
constant, a landed signed constant other than `0` or `-1`, a runtime unsigned
divisor under a direct positive lower bound, or a
runtime signed divisor under a direct positive lower
bound, `divisor <= -2` upper bound, or joint `divisor <= -1` and
`MIN + 1 <= dividend` bounds, may form decision leaves. Psi retains every exact
operation; one proof-bearing exact operation may also appear as the innermost
operation beneath up to two bitwise-not, integer-widening, or proof-free binary
shells, and distinct binary subtrees may each contain one independently proved
exact leaf. A finite same-carrier exact-add chain may instead have a direct
machine-parameter root when every non-chain sibling is a landed literal
constant. A finite same-carrier exact-subtract chain may likewise have a direct
machine-parameter root, but only the left operand continues the chain and every
right operand is a landed literal constant; reversed subtraction is not a
chain. A finite same-carrier chain may mix exact addition and subtraction when
both operation kinds occur. It continues only through each left operand from a
direct machine parameter, every right operand is a landed literal of that same
carrier, and the verifier combines additions and mathematical negations of
subtrahends in the same checked sign/magnitude offset accumulator. Every prefix
reconstructs its carrier-tight direct-root bound independently; a later
cancellation does not authorize an unsafe earlier link. A finite same-carrier
exact-multiply chain may also continue only through its left operand from a
direct machine parameter. Every right operand must be
an explicitly landed literal of that same carrier and nonnegative; zero and one
are admitted, while signed negative factors are not. A finite same-carrier chain
may also mix exact divide and remainder,
continue only through its left operand from a direct machine parameter, and use
only landed nonzero unsigned divisors or landed signed divisors other than `0`
and `-1`. For addition, subtraction, their mixed chain, and multiplication, the
verifier walks only prior left-to-right definitions with a shrinking prefix.
Addition/subtraction combine constants or mathematical negations of subtrahends
as a checked sign and magnitude and reject accumulator overflow or a magnitude
beyond the carrier span. Multiplication combines only same-carrier nonnegative right factors in a
checked `u128` accumulator. Cumulative factor zero or one is total; a larger
unsigned factor reconstructs `root <= MAX / factor`, and a larger signed factor
reconstructs both `MIN / factor <= root` and `root <= MAX / factor`. Every
earlier multiply retains its own independently checked obligation, so a later
zero never authorizes an unsafe earlier link. One unified affine-chain family
admits a finite left-associated same-carrier chain containing both at least one
exact add/subtract and at least one exact multiply. It starts at one direct
machine parameter; every right sibling is an independently landed same-carrier
literal, and multiply factors are nonnegative. For each shrinking prefix the
verifier walks ordered definitions and replays `A * root + B`: addition and
subtraction adjust the checked signed offset `B`, multiplication checked-scales
both the nonnegative coefficient `A` and `B`. If `A > 0`, the verifier maps the
carrier interval back with mathematical ceiling/floor division and intersects
it with the root carrier. If `A == 0`, the current obligation is true exactly
when `B` is carrier-representable. Every earlier prefix remains independently
proved, so later zero factors or cancellation supply no authority. Homogeneous
chains continue to use their narrower existing families.

A separate signed-affine family admits the direct chain, that chain feeding
one validator-legal partial fixed-native exact cast, and one direct partial
cast feeding the chain. The signed-carrier chain is finite, left-associated,
same-carrier, and contains both an exact add/subtract offset and at least one
negative exact-multiply factor; every right sibling is an independently landed
same-carrier literal. Ordered shrinking replay composes coefficient and offset
as checked sign/magnitude `(A, B)` for `A * root + B`. A positive coefficient
uses the ordinary interval preimage, a negative coefficient reverses the
endpoints, `MIN` is handled by magnitude, and zero decides only the current
obligation after complete shape validation. The direct, pre-cast, or post-cast
carrier intersection emits only canonical root bounds. Every arithmetic prefix
and cast retains separate evidence. Mathematical empty preimages are canonical
falsehood; coefficient, offset, division, or interval-transfer failure admits
no family. Homogeneous signed products, nonnegative affine chains, two-sided
sandwiches, multiple conversions, runtime or computed siblings, literal-left
and right-associated forms, carrier drift, locals, members, calls, effects,
stale definitions, and redirected evidence remain on narrower paths or fail
closed.

A consolidated two-sided signed-affine sandwich admits exactly one
validator-legal partial exact cast between signed fixed-native carriers, with
nonempty left-associated landed-literal add/subtract/multiply chains on both
sides. The source-qualified branch requires an offset and a negative multiply
in the source and accepts each target affine prefix. The target-qualified
branch keeps the source on the established nonnegative affine algebra and
requires the current target prefix to contain an offset and a negative
multiply. For each target obligation the verifier replays checked
sign/magnitude `(At, Bt)`, reverses a negative target preimage, intersects the
target preimage with both cast carriers, and replays checked `(As, Bs)` to the
direct source parameter, reversing again when needed. `MIN` is never host
negated. A zero target coefficient decides only target constant
representability after full source/cast validation; a zero source coefficient
decides only whether its constant lies in the surviving interval. Every source
prefix, cast, and target prefix retains separate evidence. Mathematical empty
preimages or intersections are canonical falsehood; checked coefficient,
offset, division, or interval-transfer failure admits no family. Empty sides,
all-nonnegative sandwiches, homogeneous signed products, thin product/offset
permutations, unsigned or address carriers, multiple conversions, runtime or
computed siblings, noncanonical roots, intervening operations, and stale or
redirected definitions remain on narrower paths or fail closed.

A finite same-value-carrier exact-right-shift
chain may also continue only through its left operand from a direct machine
parameter. Every right operand must be a landed literal in one of the current
fixed native signed or unsigned integer count carriers and independently satisfy
`0 <= count < value width`; count carriers may differ between links. Each
divide/remainder or right-shift obligation reconstructs independently from its
own safe landed divisor or count, so no producer-definition traversal supplies
authority. A finite same-value-carrier exact-left-shift chain may also continue
only through its left operand from a direct machine parameter. Each right
operand must likewise be an independently landed in-range fixed native integer
count, and count carriers may differ. The verifier follows only prior
left-to-right definitions with a shrinking prefix, accumulates counts in a
checked `u128`, and reconstructs every link from the cumulative count: zero is
total; `0 < cumulative < width` requires `root <= MAX >> cumulative` for
unsigned roots and `MIN >> cumulative <= root <= MAX >> cumulative` for signed
roots; cumulative counts at least the width require the root to equal zero.
One mixed shift family admits any finite left-associated same-carrier chain
containing both exact-left and exact-right shifts from a direct machine
parameter. Every count is an independently landed legal fixed-native literal;
count carriers may differ. Each right-shift proof remains only its own legal
count proposition. For each left prefix the verifier starts with that
operation's carrier-tight safe input interval and walks every prior canonical
mixed-shift definition backward: a prior left shift maps `[a,b]` to
`[ceil(a/2^k), floor(b/2^k)]`, while a prior arithmetic or zero-fill right shift
maps it to `[a*2^k, (b+1)*2^k-1]`; each step intersects the value carrier.
Empty intervals reject, full intervals are true, and surviving intervals become
canonical direct-root bounds. Every operation keeps distinct evidence, so a
later right shift cannot erase an unsafe earlier left prefix. Homogeneous shift
families remain on their existing paths. Runtime, computed,
negative, out-of-range, address, or non-native counts, mixed value carriers,
local, block, computed, or nested-cast roots, intervening shells or operations,
right-associated shapes, malformed, reordered, cyclic, redirected, or stale
definitions, interval overflow, and stale or missing evidence remain fenced.
The same mixed-only chain may feed one validator-legal partial fixed-native
exact cast. The cast proof starts from the intersection of the target and
source carriers, then walks every canonical mixed-shift definition backward
with the same inverse-left and inverse-right transfers. It emits only the
surviving canonical direct-root interval. A mathematically empty preimage is
canonical falsehood; checked interval-arithmetic failure is no admission.
Every shift-prefix proof and the cast proof remain distinct, and homogeneous
shift-to-cast chains continue to use their existing narrower paths.
Conversely, one validator-legal partial fixed-native cast of a direct machine
parameter may root the same mixed-only finite chain in the target carrier. For
each left prefix the verifier walks the shrinking canonical definitions back to
the cast, applies the same inverse-left and inverse-right transfers, intersects
the surviving target interval with the source carrier, and emits only canonical
source-root bounds. The direct cast proof, every left-prefix proof, and every
right count proof remain independently mandatory. Mathematical emptiness is
canonical falsehood; checked transfer failure admits no family. Homogeneous
post-cast shift chains stay on their narrower existing paths.
A finite ordered chain of at least two validator-legal partial fixed-native
exact casts may likewise root one nonempty computed suffix in the final target
carrier. The classifier admits only the existing affine, homogeneous signed-
product, exact-shift, or landed-safe-literal divide/remainder suffix shapes.
For each proof-bearing suffix prefix, the verifier validates the full shrinking
cast-definition walk, intersects every carrier without importing cast evidence,
and then applies only that suffix family's existing inverse transfer. Every
cast and suffix operation keeps distinct evidence. Empty mathematical
preimages become canonical falsehood; malformed definitions, runtime literals,
or checked interval failure admit no family.
The two one-sided rules compose when both sides are nonempty: an admitted
affine, homogeneous signed-product, exact-shift, or carrier-total landed-
divisor prefix may feed at least two contiguous partial fixed-native exact
casts and then an admitted affine, homogeneous signed-product, exact-shift, or
landed-safe-literal divide/remainder suffix. Every source operation, cast, and
target operation retains independent evidence. For each current obligation the
verifier walks ordered shrinking definitions, intersects the complete cast
carrier chain, applies the target family's existing inverse transfer, then the
source family's existing inverse transfer or complete-hull rule. Mathematical
emptiness is canonical falsehood; malformed shapes and checked replay failure
admit no family. Empty-sided, one-cast, direct, and narrower sandwich shapes
remain on their existing dispatch paths.
A separate total-conversion composition admits a nonempty affine, homogeneous
signed-product, exact-shift, or carrier-total landed-divisor prefix, followed by
one or more ordered strict valid fixed-native `IntegerWiden` operations and a
nonempty affine, homogeneous signed-product, exact-shift, or landed-safe-literal
divide/remainder suffix. The verifier validates every adjacent widening edge
and shrinking definition prefix. Widening preserves the mathematical integer,
so each target preimage intersects the original source carrier before the
selected source inverse or complete-hull replay. Every exact operation retains
independent evidence; every widening remains an ordinary retained executable
operation without invented proof evidence. Mathematical emptiness is canonical
falsehood, divide/remainder partial overlap and checked replay failure admit no
family, and zero remains local after complete shape validation. Direct,
widen-roundtrip, cast, and multi-cast paths keep their existing priority.
A heterogeneous conversion-spine composition requires both conversion kinds:
at least one strict valid fixed-native `IntegerWiden` and at least one
validator-legal partial `IntegerExactCast`, with at least two contiguous edges
between nonempty computed source and target families. Each cast obligation
walks its complete preceding conversion prefix, intersects every carrier, and
then reuses only the source family's established inverse or complete-hull
algebra. Target affine, signed-product, and shift obligations walk the complete
conversion word before source replay; target divide/remainder operations retain
their own landed safe-divisor proofs. Widenings are retained numeric identity,
not proof authority, and every partial cast remains independently evidenced.
Source divide/remainder may prove a cast only by complete hull containment;
partial overlap or disjointness does not manufacture cast evidence. Empty
affine/product/shift mathematics is falsehood, checked transfer failure is no
admission, and zero coefficients remain local after complete shape validation.
Pure widening, pure cast, one-edge, direct, widen-roundtrip, and narrower
sandwich shapes stay on their existing dispatch paths. Carrier drift, invalid
conversion direction, stale or reordered definitions, intervening shells,
calls, effects, local/member roots, invalid literals, arithmetic overflow, and
missing or redirected evidence remain fenced.
The unified sandwich permits nonempty finite exact-shift chains on both sides
of one validator-legal partial fixed-native cast. Counts remain independently
landed, heterogeneous, legal fixed-native literals, and either side may be
homogeneous or mixed. Each target-left prefix replays all prior target shifts
to the cast, intersects the surviving target interval with the source carrier,
then replays the complete source shift chain to a direct machine parameter.
The source shift obligations, cast obligation, and every target shift
obligation remain independent; no evidence is imported. Mathematical empty
preimages are canonical falsehood, while checked transfer failure admits no
family. Empty-sided shapes remain on their one-sided paths; runtime/computed or
illegal counts, invalid or repeated casts, carrier drift, intervening
operations, noncanonical roots, and stale or malformed definitions remain
fenced.
One unified cross-family composition admits a finite nonempty left-associated
same-carrier arithmetic prefix over exact add, subtract, and nonnegative
multiply by landed same-carrier literals, followed by a finite nonempty shift
suffix with independently landed heterogeneous legal counts and at least one
exact-left shift. For every left prefix the verifier first maps its safe input
interval backward through the prior canonical left/right shift definitions,
then composes the arithmetic definitions as checked `A * root + B` and maps the
surviving interval back to the direct machine parameter. `A == 0` decides only
that left-prefix proposition from `B`; every arithmetic-prefix proof and every
shift proof remains independently mandatory. Mathematical emptiness is
canonical falsehood, while checked affine or interval transfer failure admits
no family. Right-only suffixes, runtime or computed siblings/counts, negative
factors, non-left-associated shapes, casts, shells, and non-parameter roots
remain fenced.
The converse composition admits a finite nonempty left-associated exact-shift
prefix followed by a finite nonempty same-carrier add/subtract/nonnegative-
multiply literal suffix. Every count remains independently landed and every
left-shift overflow and arithmetic-prefix obligation remains distinct. For each
arithmetic prefix the verifier composes checked `A * shifted_root + B`, maps the
carrier backward through that affine form, then replays the complete ordered
shift prefix to the direct machine parameter. `A == 0` decides only the current
arithmetic proposition after the complete canonical root shape is validated;
it cannot erase any earlier proof. Mathematical emptiness is falsehood, while
checked affine or interval transfer failure admits no family. Runtime or
computed siblings/counts, negative factors, reversed or right-associated
shapes, casts, shells, and non-parameter roots remain fenced.
One unified cast sandwich admits a finite nonempty source-carrier affine chain,
one validator-legal partial fixed-native exact cast, and a finite nonempty
target-carrier affine chain. Each side may use any left-associated sequence of
exact add, subtract, and nonnegative multiply by independently landed
same-carrier literals. The cast independently maps the target/source
intersection through the checked source form `As * root + Bs`. Each target
prefix independently maps the target carrier through `At * cast_value + Bt`,
intersects that preimage with the source carrier, then maps it through the full
source form to the direct parameter. A zero coefficient on either side decides
only the current proposition after the complete ordered sandwich is validated;
it cannot erase any source-prefix, cast, or earlier target-prefix proof.
Mathematical emptiness is falsehood, while checked composition or interval
failure admits no family. Empty sides stay on narrower existing paths; carrier
drift, runtime or computed siblings, negative factors, intervening operations,
nested casts, and non-parameter roots remain fenced.
One consolidated heterogeneous sandwich admits either a finite nonempty source
affine chain followed by one validator-legal partial fixed-native exact cast
and a finite nonempty target shift chain, or a finite nonempty source shift
chain followed by the cast and a finite nonempty target affine chain. Affine
chains retain independently landed same-carrier add/subtract/nonnegative-
multiply literals; shift chains retain independently landed heterogeneous
legal counts. Each target-left obligation or target-affine prefix replays its
own ordered definitions to the cast, intersects target and source carriers,
then replays the complete source affine or shift prefix to the direct machine
parameter. Every source operation, cast, and target operation remains an
independent proof obligation. A zero affine coefficient decides only the
current obligation after complete shape validation. Mathematical empty
preimages are canonical falsehood; checked composition, count accumulation, or
interval-transfer failure admits no family. Empty-sided shapes remain on their
narrower paths. Carrier drift, runtime or computed affine siblings/counts,
negative factors, invalid or repeated casts, intervening operations,
noncanonical roots, and stale or malformed definitions remain fenced.
One consolidated divide/remainder cross-cast family admits all four nonempty
compositions between a landed-literal exact-divide/remainder chain and an
affine or shift chain across one validator-legal partial fixed-native exact
cast. When divide/remainder precedes the cast, the verifier replays the complete
source chain from the full source carrier using toward-zero quotient and
dividend-sign remainder hull transfer. That hull must lie wholly in the target
carrier. Each target affine prefix or target-left prefix then reconstructs its
own safe target interval by the established checked affine or ordered shift
replay: a hull wholly inside that interval is truth, a disjoint hull is
canonical falsehood, and partial overlap admits no family because it would
require a guard-sensitive nonconvex source preimage. A zero target affine
coefficient decides only its current proposition after the complete source,
cast, and target shape is validated. In the converse direction, source affine
or shift chains and the cast use their existing independent reconstruction,
while every target divide/remainder operation depends only on its own landed
safe divisor. Every source operation, cast, and target operation retains
separate evidence. Zero, signed `-1`, runtime, computed, or mistyped divisors;
runtime or computed affine siblings or counts; negative factors; invalid,
widening, or repeated casts; carrier drift; empty sides; intervening
operations; nonparameter roots; malformed definitions; and checked transfer
failure remain fenced. Existing narrower and runtime-divisor families are
unchanged.
The same four divide/remainder-to-affine/shift compositions are admitted
directly, without a cast, when both nonempty chains share one fixed-native
carrier and the innermost root is a direct machine parameter. When
divide/remainder comes first, the verifier replays its complete carrier-total
hull and compares that hull with each target affine or target-left safe input
interval. Complete containment is truth, disjointness is canonical falsehood,
and partial overlap remains unadmitted. A zero affine coefficient decides only
its current prefix after the complete divide/remainder shape is validated.
When affine or shift comes first, its established direct-root proof replay is
unchanged and each following divide/remainder operation depends only on its own
landed safe divisor. Every operation retains separate evidence. Both sides must
be nonempty; casts, runtime or computed divisors/siblings/counts, unsafe or
mistyped divisors, negative factors, carrier drift, intervening operations,
nonparameter roots, stale definitions, and checked replay failure remain
fenced. Existing narrower and cross-cast families are unchanged.
A separate two-sided sandwich admits a finite nonempty landed-literal exact-
divide/remainder chain, one validator-legal partial fixed-native exact cast,
and a finite nonempty target exact-divide/remainder chain. Both sides use
ordered left-associated same-carrier definitions and independently landed
safe divisors. The cast replays the complete source carrier through the source
quotient/remainder hull transfers and is admitted only when that hull wholly
fits the target carrier; failure to prove full containment admits no family
rather than constructing a partial or false proposition. Every source
operation, the cast, and every target operation retains independent evidence,
and each target proposition uses only its own divisor. Empty sides remain on
the existing one-sided paths. Runtime, computed, zero, signed `-1`, or mistyped
divisors; carrier drift; invalid, widening, or repeated casts; intervening
operations; nonparameter roots; malformed definitions; and stale evidence
remain fenced.
One same-root affine fork/join admits an outer exact add or subtract with two
nonempty proof-bearing operands. Each operand must be a disjoint, independently
admitted direct landed-literal affine branch on the same fixed-native carrier,
and both branch walks must terminate at the exact same machine-signature
parameter. Terminal production retains the complete left branch, then the
complete right branch, then the join. The verifier walks those definition sets
separately, requires them to be disjoint and source ordered apart from their
common root, and composes checked sign/magnitude `Al * root + Bl` and
`Ar * root + Br`. The join carrier is pulled back through the sum or difference
of those forms. A zero combined coefficient decides only the join after both
branches validate; every branch operation remains independently evidenced.
Mathematical empty preimages are falsehood, while checked composition failure
admits no family. One empty branch, distinct roots, carrier drift, literal-left
or right-associated forms, runtime or computed branch siblings, conversions,
outer operations other than add/subtract, locals, members, calls, effects,
overlapping or reordered definitions, and stale or redirected evidence remain
fenced. Existing direct, linear, cast, and conversion families keep priority.
One distinct-root signature-bounded affine fork/join admits the same outer
fixed-native exact add or subtract when its two nonempty landed-literal affine
branches have disjoint source-ordered definition walks and terminate at two
different direct machine-signature parameters of the same carrier. For each
root, the verifier selects only the tightest landed unary lower and upper
signature bounds, intersects them with the carrier, and maps the interval
forward through the branch's checked signed affine form. It forms the outer
range by Minkowski addition or subtraction. Complete containment in the join
carrier emits the canonical conjunction of the selected bounds; a wholly
disjoint range emits falsehood; partial overlap admits no family. Relational
cross-root premises, missing or one-sided unary bounds, shared or computed
roots, carrier drift, overlapping or reordered definitions, conversions, and
checked interval failure remain fenced. Every operation in both branches and
the join retains independent evidence, and existing narrower families retain
priority.
One distinct-root signature-bounded signed affine product join admits an outer
fixed-native exact multiply when its two nonempty landed-literal affine
branches have disjoint source-ordered definition walks and terminate at two
different direct signed machine parameters. Both roots must retain landed
unary lower and upper signature bounds. The verifier selects the tightest
endpoints, maps them through the checked signed affine branches, and forms the
exact interval hull of all four corner products. Complete containment emits
the canonical four-bound conjunction, a wholly disjoint hull emits falsehood,
and partial overlap or checked corner multiplication failure admits no family.
Every branch operation and the outer multiply retains independent evidence.
Same-root quadratic correlation, relational premises, one-sided bounds,
unsigned carriers, carrier drift, computed roots, conversions, overlapping or
reordered definitions, and stale evidence remain fenced. Existing constant-
factor, runtime-factor, chain, conversion, and add/subtract fork families keep
priority.
One same-root signature-bounded signed affine quadratic product join admits an
outer fixed-native exact multiply when its two nonempty landed-literal affine
branches have disjoint source-ordered definition walks, nonzero coefficients,
and terminate at the same direct signed machine parameter. The root must
retain landed unary lower and upper signature bounds. The verifier selects the
tightest endpoints, composes the correlated checked integer quadratic, and
evaluates its exact discrete range at both interval endpoints plus the
in-range floor and ceiling adjacent to the rational vertex. Complete
containment emits the canonical two-bound conjunction, a wholly disjoint range
emits falsehood, and partial overlap or checked coefficient, vertex, or
evaluation failure admits no family. Every branch operation and the outer
multiply retains independent evidence. Constant collapse, distinct or
computed roots, relational premises, one-sided bounds, unsigned carriers,
carrier drift, conversions, overlapping or reordered definitions, and stale
evidence remain fenced. Existing constant-factor, runtime-factor, chain, cast,
and affine families keep priority; this correlated family precedes the
distinct-root product rectangle.
One same-root signature-bounded signed affine divide/remainder safety join
admits an outer exact divide or remainder when its two nonempty
landed-literal affine branches have disjoint source-ordered definition walks
and terminate at the same direct signed fixed-native signature parameter with
nonzero coefficients. The verifier selects only the tightest unary signature
lower and upper bounds, then solves the divisor's exact integer-lattice zero
and `-1` equations. A divisor `-1` root is unsafe only when the correlated
dividend equals the carrier minimum at the same root. No forbidden root emits
the canonical two-bound conjunction; forbidden roots covering the complete
integer interval emit falsehood; partial safety or checked arithmetic failure
admits no family. Every operation in both branches and the outer operation
retains independent evidence. Bounds are read only from the retained machine
signature, never from operation-definition axioms. Distinct roots, relational
or one-sided premises, unsigned carriers, constant collapse, computed roots,
conversions, malformed walks, and stale evidence remain fenced. Existing
literal, direct-runtime, chain, cast, and carrier-total divide/remainder forms
retain priority.
Terminal retains every operation and obligation, and every
operation's evidence is checked independently. Two computed operands outside
the admitted affine fork/join, affine quadratic product-join, distinct-root
affine product-join, and same-root affine divide/remainder safety-join families,
nonconstant siblings, runtime or computed multiply factors or shift counts,
signed negative multiply factors, right-associated or reversed shapes, local or
block-parameter roots, exact operations outside the admitted chain family, and
other proof-bearing compositions remain fenced. For addition, subtraction,
their mixed offset chain, multiplication, the mixed affine chain, and left
shift, missing, reordered, reversed, redirected, cyclic, or stale definitions
reject. The affine family additionally rejects coefficient or offset
composition overflow. For every family, stale
operation/factor/divisor/count evidence and missing evidence reject. Multiply
and left shift additionally reject cumulative arithmetic
overflow. One
signed-product widening applies only to the three homogeneous exact-multiply
placements: a direct chain, a chain feeding one partial fixed-native exact
cast, or a chain rooted at one direct partial cast. The signed carrier chain
must contain at least one negative independently landed right factor. Ordered
shrinking definitions accumulate the mathematical product as checked
sign/magnitude, so `MIN` needs no host negation; a negative product reverses
the target interval before carrier intersection. Zero makes only the current
proposition true, and every earlier multiply and cast retains separate
evidence. Mathematical empty preimages are falsehood; checked product or
division failure admits no family. Unsigned/nonnegative and mixed affine paths
remain unchanged, while runtime or computed factors, literal-left and
right-associated forms, carrier drift, additional operations or casts,
nonparameter roots, stale definitions, and redirected evidence remain fenced.
One cast-only composition admits a finite chain of at least two partial
fixed-native exact casts rooted at a direct integer machine parameter. Every
adjacent edge remains independently validator-legal; for each prefix the
verifier walks only ordered shrinking result definitions and intersects the
root carrier with every source and target carrier reached so far. The
canonical surviving root bounds prove only the current cast, so no earlier
cast proof or evidence is imported. Mathematical empty intersection is
falsehood; malformed, reordered, cyclic, mistyped, widening, same-type, local-
rooted, intervening-operation, missing-evidence, or redirected-evidence shapes
remain fail-closed. The direct one-cast and widen-then-narrow paths are
unchanged.
The finite cast core may instead follow one nonempty computed prefix from an
already-admitted pre-cast family: same-carrier landed-literal affine arithmetic,
the homogeneous signed-product path, a homogeneous or mixed exact-shift chain,
or a carrier-total landed-literal exact-divide/remainder chain. At least two
partial fixed-native casts remain required for this wider family. For each
cast prefix the verifier walks ordered shrinking cast definitions, intersects
every carrier reached so far, then applies only the selected source family's
existing inverse algebra to the direct machine-parameter root. A zero affine
coefficient or product decides only the current cast; negative products reverse
the complete carrier intersection; shifts replay every ordered inverse step;
and divide/remainder is admitted only when its complete verifier-owned hull is
contained. Every source operation and cast retains distinct evidence.
Mathematical empty affine/product/shift preimages are falsehood, while checked
composition or interval failure admits no family. Empty prefixes, fewer than
two casts, post-cast operations, cross-family prefixes, invalid cast edges,
runtime siblings, nonparameter roots, malformed definitions, and stale or
redirected evidence remain fenced. Direct, one-cast, sandwich, and cast-only
paths are unchanged.
One
separate computed-cast exception accepts a direct
fixed-integer parameter
widened through any finite chain of valid fixed-carrier widenings and then
exactly narrowed back to its original carrier. Terminal retains every ordered
`IntegerWiden` and the `IntegerExactCast`. The verifier walks only prior
left-to-right value definitions, reduces the available definition prefix at
every step, checks every adjacent carrier and strict widening, and requires the
origin value to be a machine signature parameter of the narrowing's target
carrier. The walk is bounded by the finite prior-axiom count; missing,
reordered, reversed, cyclic, mistyped, or redirected definitions reject the old
self-proof. Local or block-parameter roots and otherwise computed exact casts
remain fenced. A second computed-cast exception accepts
one partial exact conversion whose operand is a finite nonempty left-associated
same-carrier exact-add/subtract chain. The chain uses a current fixed-native
source carrier, starts at one direct machine parameter, and has one independently
landed same-carrier literal on every right edge. The target is another current
fixed-native carrier; same-width and cross-sign partial conversions are
included. The verifier follows only ordered shrinking-prefix definitions,
accumulates additions and mathematical negations of subtrahends in the checked
sign/magnitude offset, and reconstructs the cast as the target interval shifted
back by that offset and intersected with the source carrier. Vacuous sides are
omitted, an empty intersection is false, and one or two surviving bounds are
canonical source-carrier propositions. Every arithmetic prefix retains its own
obligation and evidence, so cancellation or a cast-safe final interval cannot
erase an earlier unsafe operation. Computed or unlanded siblings, literal-left
addition, reversed subtraction, right-associated shapes, local or block roots,
mixed-carrier or non-native chains, other proof-bearing operations, additional
casts, missing or noncanonical definitions, accumulator overflow, and stale or
missing evidence remain fenced. A third computed-cast exception accepts one
validator-legal partial fixed-native exact cast whose operand is a finite
nonempty left-associated same-source-carrier exact-multiply chain. The chain
starts at one direct machine parameter and every right operand is an
independently landed nonnegative source-carrier literal. Every multiply prefix
retains its ordinary independent obligation and evidence. For the cast, the
verifier follows only prior canonical shrinking-prefix definitions, accumulates
the factors in a checked `u128`, maps the target range back through the
cumulative product, and intersects it with the source carrier. Product zero
makes only the cast obligation true. Product one uses the ordinary target/source
intersection. A larger product reconstructs `[0, MAX / product]` for an unsigned
target or `[ceil(MIN / product), floor(MAX / product)]` for a signed target
before the source-carrier intersection; vacuous sides are omitted and an empty
intersection is false. Literal-left or right-associated shapes, runtime,
computed, negative, or mistyped factors, mixed carriers, local or block roots,
intervening operations or casts, non-native or invalid casts, malformed or stale
definitions, cumulative-product overflow, and stale or missing evidence remain
fenced. A later zero cannot erase an earlier multiply proof. A fourth
computed-cast exception accepts one validator-legal partial fixed-native exact
cast whose operand is a finite nonempty left-associated same-source-carrier
exact-left-shift chain rooted at one direct machine parameter. Every right
operand is an independently landed legal fixed-native count, and count carriers
may differ. The verifier follows only prior canonical shrinking-prefix
definitions, checked-adds the counts, maps the target interval right by the
cumulative count, and intersects it with the source carrier without importing
any shift-prefix evidence. Count zero uses the ordinary target/source
intersection. A positive count below the source width reconstructs
`[0, MAX >> count]` for an unsigned target or
`[ceil(MIN / 2^count), floor(MAX / 2^count)]` for a signed target before the
source intersection. At or above the source width, the cast alone is true
because any successfully produced exact source result is zero; every shift
prefix still retains its independent carrier-safety or zero-root proof.
Runtime, computed, negative, out-of-range, address, or non-native counts,
right-associated shapes, mixed value carriers, local or block roots,
intervening operations or casts, non-native or invalid casts, malformed or
stale definitions, cumulative-count overflow, and stale or missing evidence
remain fenced. A fifth computed-cast exception accepts the corresponding finite nonempty
same-source-carrier exact-right-shift chain. Counts, root, definition walk, and
fences match the pre-cast left-shift family, but every shift-prefix obligation
remains only its independent legal-count proof. For cumulative count `C` below
the source width, with `Q = 2^C`, the cast maps target interval `[L, U]` back to
`[L*Q, (U+1)*Q-1]` and intersects the result with the source carrier. At or
above source width an unsigned source yields zero; a signed source yields
`-1` or `0`, so the cast is true for a signed target and requires `0 <= root`
for an unsigned target. No shift proof is imported into the cast reconstruction.
A further computed-cast family accepts a finite nonempty left-associated
same-source-carrier exact-divide/remainder chain rooted at one direct machine
parameter when its result is carrier-total for the partial cast. Every right
sibling is an independently landed same-carrier safe divisor. The verifier
walks only prior canonical shrinking-prefix definitions, then replays them
inner-to-outer from the full source-carrier interval: toward-zero division maps
endpoints monotonically (reversing them for a negative divisor), while
remainder uses the dividend-sign interval hull clipped by `abs(divisor) - 1`.
The family is retained only when the final hull lies wholly inside the target
carrier. No guard-sensitive or nonconvex preimage, operation proof, or evidence
is imported into the cast; every divide/remainder prefix and the cast retain
independent evidence. Noncontained hulls, zero, signed `-1`, runtime, computed,
or mistyped divisors, literal-left or right-associated shapes, mixed carriers,
local or block roots, intervening operations or casts, non-native, identity,
widening, or invalid casts, malformed, stale, or out-of-order definitions,
interval arithmetic failure, and stale or missing evidence remain fenced.
A further computed-cast exception accepts the unified finite left-associated
same-source-carrier mixed affine chain described above when it contains both an
exact add/subtract and an exact multiply. The cast is validator-legal and
partial, the root is one direct machine parameter, every right sibling is an
independently landed same-carrier literal, and multiply factors are
nonnegative. The verifier follows only prior canonical shrinking-prefix
definitions and replays the full operand as `A * root + B` with checked
coefficient and offset composition. For `A > 0`, it maps the target carrier
back to `[ceil((TARGET_MIN-B)/A), floor((TARGET_MAX-B)/A)]` and intersects that
interval with the source carrier. For `A == 0`, only the cast is true exactly
when `B` is target-representable. No arithmetic-prefix proof is imported into
cast reconstruction, so later zero or cancellation cannot erase an earlier
obligation. Homogeneous chains remain on their narrower computed-cast paths.
Literal-left, reversed, or right-associated shapes, runtime, computed,
negative, or mistyped factors/siblings, mixed or non-native carriers, local or
block roots, intervening shells, operations, or casts, invalid or widening
casts, malformed or stale definitions, coefficient/offset overflow, and stale
or missing evidence remain fenced.
Conversely, one
validator-legal partial fixed-native exact cast of a direct machine parameter
may root a finite
nonempty left-associated same-target-carrier exact-add/subtract chain. The cast
result is the innermost left operand, and every right operand is an
independently landed target-carrier literal. The cast retains its ordinary
direct source-to-target representability obligation. For every arithmetic
prefix, the verifier walks only prior canonical shrinking-prefix definitions
through the chain to the cast, accumulates additions and mathematical negations
of subtrahends with checked sign/magnitude arithmetic, shifts the target
interval back by that cumulative offset, and intersects it with the source
carrier. The cast and every arithmetic prefix retain distinct obligations and
evidence, so later cancellation cannot erase earlier safety. Literal-left or
reversed arithmetic, runtime or computed siblings, right-associated shapes,
local or block roots, intervening shells, additional casts or other
proof-bearing operations, non-native or mismatched carriers, missing,
reordered, reversed, redirected, cyclic, or stale definitions, cumulative
offset overflow, and stale or missing evidence remain fenced.
A direct validator-legal partial fixed-native exact cast may likewise root a
finite nonempty left-associated same-target-carrier exact-multiply chain. Every
right operand is an independently landed nonnegative target-carrier literal;
signed negative factors remain outside this family. The cast independently
proves direct representability. For each multiply prefix, the verifier walks
only prior canonical shrinking-prefix definitions to that cast and accumulates
the literal factors with checked arithmetic. Cumulative product zero or one
makes only the current multiply prefix true. A larger product divides the
target interval back toward the direct source root—`[0, MAX / product]` for an
unsigned target, or `[ceil(MIN / product), floor(MAX / product)]` for a signed
target—and intersects that interval with the source carrier. Vacuous sides are
omitted and an empty intersection is false. The cast and every prefix retain
distinct obligations and evidence, so a later zero factor cannot erase an
earlier unsafe multiply. Literal-left or right-associated shapes, runtime,
computed, negative, or mistyped factors, mixed carriers, local or block roots,
intervening operations or casts, non-native or invalid casts, malformed or
stale definitions, cumulative-product overflow, and stale or missing evidence
remain fenced.
A direct validator-legal partial fixed-native exact cast may instead root the
unified finite nonempty left-associated same-target-carrier affine chain when
both an exact add/subtract offset and an exact multiply occur. Every right
sibling is an independently landed target-carrier literal, and multiply
factors are nonnegative. The cast retains its independent direct
representability proof. For every arithmetic prefix, the verifier follows only
prior canonical shrinking-prefix definitions to the cast and composes the
checked affine form `A * source + B`. Positive `A` maps the target interval
back through ceiling/floor division and intersects it with the source carrier;
`A == 0` makes only the current prefix true or false from target
representability of `B`. No cast or earlier arithmetic evidence is imported,
so later zero factors or cancellation cannot erase an earlier obligation.
Homogeneous offset and multiply chains remain on their narrower paths.
Literal-left, reversed, or right-associated shapes, runtime, computed,
negative, or mistyped siblings, mixed carriers, local or block roots,
intervening operations or casts, non-native or invalid casts, malformed or
stale definitions, coefficient/offset overflow, and stale or missing evidence
remain fenced.
A direct validator-legal partial fixed-native exact cast may also root a finite
nonempty left-associated same-value-carrier exact-left-shift chain. Every right
operand is an independently landed fixed-native signed or unsigned count,
count carriers may differ between links, and each count independently satisfies
`0 <= count < value width`. The cast independently proves direct
representability. For each shift prefix, the verifier walks only prior
canonical shrinking-prefix definitions to that cast and accumulates counts in
a checked `u128`. Cumulative count zero makes only the current shift prefix
true. A positive cumulative count below the value width shifts the target
interval right by that count—`[0, MAX >> count]` for an unsigned target or
`[MIN >> count, MAX >> count]` for a signed target—and intersects it with the
source carrier. A cumulative count at least the width intersects the zero-only
target interval with the source carrier. Vacuous sides are omitted and an empty
intersection is false. The cast and every prefix retain distinct obligations
and evidence. Runtime, computed, negative, out-of-range, address, or non-native
counts, right-associated or reversed shapes, mixed value carriers, local or
block roots, intervening operations or casts, non-native or invalid casts,
malformed or stale definitions, cumulative-count overflow, and stale or
missing evidence remain fenced.
A direct validator-legal partial fixed-native exact cast may also root the
corresponding finite nonempty left-associated same-value-carrier
exact-right-shift chain. Counts are independently landed legal fixed-native
literals and their carriers may differ. The cast independently proves direct
representability; every shift prefix independently reconstructs `Truth` from
its own `0 <= count < width` fact. Unlike left shift, no cumulative count,
value-definition traversal, source interval, cast evidence, or earlier shift
proof is needed. Runtime, computed, negative, out-of-range, address, or
non-native counts, right-associated or reversed shapes, mixed value carriers,
local or block roots, intervening operations or casts, non-native or invalid
casts, malformed or stale definitions, and stale or missing evidence remain
fenced.
A direct validator-legal partial fixed-native exact cast may also root a finite
nonempty left-associated same-target-carrier chain containing exact divide and
remainder in any order. Every right sibling is an independently landed
same-carrier safe divisor: nonzero for unsigned carriers, and neither zero nor
`-1` for signed carriers. The cast keeps its independent direct
representability proof, while every divide/remainder prefix independently
reconstructs `Truth` from only its own safe divisor. No cast evidence, prior
operation proof, value-definition walk, quotient/remainder algebra, or
cumulative state is imported. Runtime, computed, zero, signed `-1`, or mistyped
divisors, literal-left, reversed, or right-associated shapes, mixed, address,
or non-native carriers, local or block roots, intervening operations or casts,
invalid casts, malformed or stale definitions, and stale or missing evidence
remain fenced.
The direct-root and post-cast divide/remainder families admit one unified
runtime-divisor widening when at least one right sibling is a direct
same-carrier machine parameter. The direct-root form remains a nested chain of
at least two operations; the post-cast form remains nonempty. Every other right
sibling is either another direct same-carrier parameter or a landed safe
literal. Each runtime divisor independently requires `1 <= divisor` or, for a
signed negative divisor, `divisor <= -2`. Only the first direct-root operation
may instead use the joint `divisor <= -1` and `MIN + 1 <= dividend` form, and
only when the verifier independently reconstructs that direct dividend bound.
Computed and post-cast dividends cannot borrow that authority. The cast and
every operation retain distinct evidence; no quotient/remainder value
definition or earlier proof is imported. Literal-only chains keep their
existing paths. Zero, signed `-1`, local, block, computed, mistyped, or
wrong-carrier divisors, missing divisor guards, computed or local roots,
literal-left, reversed, or right-associated shapes, intervening shells,
operations, or casts, invalid casts, malformed definitions, and stale or
missing evidence remain fenced.
All native targets join those leaves into the same cleanup tail. Nested paths,
field-only trees, a second field identity, erased or non-Boolean fields, nested
or partial integer computation, member/comparison mixtures, calls, effects,
nested nominal ownership, other projections, and wider cleanup shapes still
fail closed.

Author-declared hardware geometry is semantic and may contain offsets, widths,
and alignment. Omega begins where the target chooses native layout, stack and
register placement, ABI classes, concrete storage regions, instructions, and
relocations.

## Psi operation definition

Every operation enters the vocabulary as one reviewed vertical slice:

```text
operation identity and canonical encoding
execution transition
generated obligations and authorized admissions
proof rule / logical interpretation
soundness proof of that rule against the transition
interpreter realization
Omega lowering requirement
fuel identity
```

Operations are statically distinct when execution semantics or generated
obligations differ. Obligation-affecting policy is a closed instruction variant,
not an ordinary value that requires constant folding before verification.
Additional sound proof lemmas may be published without changing operation or
program identity.

### Direct scalar call slice

The current `Call` operation names one canonical callee, carries positional
scalar arguments, carries exactly one caller obligation identity for each
published callee `requires` clause, and explicitly records the normalized
no-successor crash continuations that survive at that invocation. Validation
checks the complete signature, argument definedness and types, result type,
obligation arity, global obligation uniqueness, and crash-continuation
coverage. Verification substitutes the positional arguments into the callee
requirements and guarantees: requirements become caller proof obligations,
while verified guarantees enter the caller's normal-return semantic axioms.

The call verifier accepts exact unconditional and guarded routes from an
in-module callee. Terminal crash predicates retain canonical proposition terms,
not producer-authored identity bytes. The verifier substitutes every callee
parameter `ValueId` with the corresponding arbitrary caller-local argument,
reconstructs the surviving continuation set, and requires coverage by the
caller's published ceiling; an empty or untranslated set therefore cannot erase
a crash. Checked scalar contracts and body crash sites retain structured
predicate meaning through terminal lowering. Invocation-specific guarded call
rows now retain that same structure after substituting direct parameter and
caller-local scalar arguments. Checked scalar graphs also retain direct
call-valued bindings, their exact call coordinate, and positional scalar
argument plans. Source production composes the reachable in-module checked
scalar call closure, consumes each matching crash row, and emits `Call` with
parameter or computed direct-local substitutions intact. Calls stage
short-circuit scalar arguments left-to-right and Omega target lowering accepts
the resulting calls inside conditional control. A guarded staged call follows
the checked row's pinned target contract and substitutes its parameter-relative
routes with the exact terminal argument values; it never reverse-matches caller
expressions, which would be ambiguous for equal or overlapping arguments. A
selected boundary-operator result bound inside an attached Unit machine uses
the same scalar `Call` operation. Provider settlement supplies the exact
authored use, public requirement, strong `ProviderPlan` identity, and selected
machine/state application while Unit planning still owns the unrewritten
operator expression. Attached-Unit production admits only that selected scalar
graph and its ordinary scalar-call closure, emits the selected realization as
a distinct Terminal machine, and passes the call result through a
source-ordered prefix of immutable, branch-free scalar locals to later calls.
Each such local rejoins its exact checked scalar-expression fact; nested calls
and short-circuit control remain outside this lane.
The first native continuation is deliberately narrower than the semantic call
surface: attached Unit bodies may call service-free fixed 8/16/32/64-bit
integer functions with constants or prior scalar-call results. Target
selection retains the callee's exact ABI; assignment gives every result a
distinct durable Unit-frame home; native emission retains typed relocation and
argument/result intervals; object construction independently regenerates those
bytes; installation format 46 transports the validated records. This is
artifact consistency, not evidence that a human or model audited the code.
Before Terminal or native production, Omega independently resolves the
carrier's strong plan identity against its complete retained
`SelectedProviderPlanFacts` and requires the row's exact checked-adapter machine
and state. A second conforming machine with compatible signature, contract, and
reach cannot replace that selected row. Missing, duplicate, stale, or
coordinate-mismatched applications fail closed; neither planning nor
production scans conformances for a uniquely shaped realization.

Free scalar return calls remain in their authored arms for computation planning.
Other supported guarded return calls use the existing arm-local continuation
normalization.
The continuation captures referenced state parameters and explicitly typed
primitive local values, including the current value of mutable local storage.
It evaluates the original scalar arguments and invokes the callee only after
the arm is selected. Repeated references share one captured value; argument
order, short-circuit scalar expressions, and partial arithmetic stay inside the
selected continuation. Local places, borrows, recasts, and nested calls are not
converted to value captures by this normalization. No additional Terminal
call or return representation is introduced.

Call-bearing scalar initializers, local assignments, state arguments, guards,
and returns retain arena-backed checked computation plans, separate from the
pure scalar expressions consumed by proof.
Their value leaves use the source state's checked scalar namespace; call nodes
retain exact flow-call handles and authored occurrence ordinals; conditional
nodes select one result; pure application templates consume only their computed
operands. Builtin Boolean composition, fixed-integer arithmetic/comparisons,
and nested free scalar calls lower to private typed blocks. Each completed
argument is carried before the next one starts, and only the selected `&&`/`||`
RHS executes. Trailing returns, unconditional value transitions, and both guarded
return arms use the same evaluation path, then deliver the completed result to
a private return block. No source state, source local, or new Terminal operation
is manufactured. Each root node retains the exact authored outer expression
handle, checked against its statement and destination role before expansion.
Invalid handles, cycles, duplicate roots/call occurrences, swapped arm roots,
and mismatched carriers or invocation coordinates reject before publication.

Pure scalar roots use the same authored-expression locator as computation roots.
Each pure plan requires one source-binding row and one expression row at its
state, statement, and destination role. Their expression handle, declaration
destination, primitive carrier, and ordered declaration namespace must rejoin
the authored source. The namespace contains scalar parameters followed by prior
initialized immutable primitive locals; mutable storage keeps its separate
symbol identity. Missing or conflicting rows, stale handles, reordered symbols,
and swapped continuation roles reject instead of selecting a matching row from
an ambiguous set. Shared structural and attached-Unit scalar-root lowering uses
this same check.
Direct scalar call bindings also rejoin the authored callee and result carrier,
and state transfers rejoin their selected target, including zero-argument calls
and transfers. A trait-operator realization consumes its existing source-bound
return only when it agrees with the selected realization plan; lowering does not
clone the checked program to insert a second return row.

Boundary and ordinary Unit calls retain source-bound scalar arguments through
their shared checked producer and all existing direct and composed lowering
paths. Each copied operation argument must agree with the unique checked plan,
authored operand handle, primitive formal, and caller declaration namespace.
The dense scalar callee ordinal is distinct from the authored argument index:
structural arguments advance the latter, and an implicit receiver does not.
Outer call coordinates and targets rejoin even when there are no scalar
arguments. Boundary source sites and exact signature or machine-parameter
requirements remain distinct from ordinary machine owners. A call through a
nominal machine parameter uses its exact boundary requirement's argument role,
not the ordinary Unit-call role of an unresolved callable. The checker and
lowerer share the existing compiler-intrinsic boundary-recognition policy;
lowering does not independently infer a compatible provider.
The retained call roots are statement calls and bare calls in immutable local
initializers or expression statements. Unit and boundary statement arguments
select either a pure expression or an exact checked computation root. Nested
scalar calls belong to those operand graphs, not additional statement operations.
The outer call keeps ordinal zero; nested preorder ordinals identify occurrences
and never determine execution order. Flow calls retain generational authored
expression handles, so lowering rejoins each nested target and operand root
without source-span lookup or a second call-ordinal traversal.
Nested calls may use a resolved data qualifier when the callee has no `self`
parameter. The qualifier must name that callee's exact attached data owner;
it is not a runtime receiver, and an arbitrary value receiver is not discarded.

Statement-call operands share the scalar evaluator's ordered private blocks.
Each completed scalar argument is retained before the next starts, including
pure Boolean short-circuit siblings. The caller's immutable scalar namespace
survives the completion block without gaining temporary argument slots. Structural
places remain in the enclosing Unit machine until the outer call commits their
transfers; normal cleanup follows existing ownership, while Trap and Abort have
no cleanup successor. Arithmetic obligations are finalized on the completed
selected Unit module, after borrowed-place and cleanup assembly, not on a
provisional closure used by another lowerer.

Single-state Unit bodies may finish with an authored Unit call expression,
with omitted or explicit Unit return annotations. Its computed scalar operands
use the same call-argument roles and evaluator as a semicolon call. Normal Unit
return follows the completed call; this does not imply stack-frame reuse or
tail-call optimization. Source normalization keeps the expression instead of
manufacturing an untyped result local. Validation requires the exact callee to
return Unit, and lowering rejoins the captured outer expression even when it
has no scalar operands. Value-returning callees cannot be silently discarded.
An affine-local prefix remains established in entry and declaration order while
operand evaluation branches. The verifier requires that exact establishment
sequence and checks live custody and reverse cleanup on each normal path; a
crash has no cleanup successor. This does not admit branch-local establishment.

Immutable scalar bare-call result initializers in a single-state Unit caller
also evaluate scalar operands through those blocks. Ordinary and boundary scalar
results may follow other scalar locals and Unit or boundary call statements.
The producer walks those statements in source order. Statement positions include
intervening calls; dense scalar-binding positions advance only when a local is
established. Pure primitive locals use the same positional distinction.

Operand roots use the pre-initializer namespace; only successful outer-call
completion establishes the result. The evaluator retains earlier values across
successive operand graphs. The result operation owns the outer call, so no
duplicate whole-initializer computation is emitted. Source eligibility and
lowering rejoin the exact local initializer, result carrier, declaration namespace,
callee, static qualifier, and captured outer flow occurrence independently from
the nested operand occurrences. Boundary structural-result initializers share
the authored statement sequence and may follow scalar locals, scalar results,
ordinary calls, and earlier structural results. Scalar and structural result
bindings retain independent dense ordinals. Each boundary result is installed
only after its operands and provider complete; unused affine results are cleaned
in reverse production order on normal return. A later operand crash retains
earlier results without creating a cleanup successor. The exact authored local,
result carrier, and call occurrence rejoin independently of operand graphs.
Whole, owned, claim-free affine boundary results may move once into a later
ordinary Unit call, scalar-returning boundary wrapper, affine-result call, or
direct boundary call. Direct boundaries accept established ordinary or boundary
results across Unit, scalar, and structural return carriers, including nominal
boundary requirements. Successful boundary completion consumes the input before
establishing a replacement result; rejected or ill-typed provider responses
leave the input available for retry.
The call rejoins the exact authored local or temporary expression and transfer
event; only that result
loses caller cleanup. Boundary result signatures remain independently checked,
without deriving facts from provider implementations. Unrestricted results do
not enter this affine move route. Named immutable, whole, plain-owned affine
results may also supply shared reads to ordinary Unit calls, scalar boundary
wrappers, and direct Unit or scalar boundary calls. The exact authored `&local`
and captured read access rejoin the producer binding; a read neither transfers
ownership nor removes cleanup. Repeated reads retain the same value identity
until one final owned move or reverse-order caller cleanup. Independent frontier
verification requires the intact owner to be live at every read and rejects a
call that both borrows and moves that result. The shared parameter is unrestricted;
the produced value and its caller-owned custody remain affine. Provider refusal
leaves that custody available for retry, and a later operand crash adds no cleanup
successor. An ordinary Unit call may also read a whole anonymous result
through its sole `&producer(...)` argument. The source owner retains the real
expression root: owned affine establishment at the producer, an unrestricted
shared loan at the consumer, then affine cleanup at that consumer's dying
continuation. Lowering independently rejoins those events, their provenance,
source order, and exact call identities. No source local is synthesized; the
existing result binding remains live through the read. A checked
`CallContinuationCleanup` entry immediately after the consumer names its exact
call coordinate and ordered affine discard rows. Lowering emits an ordinary
`Jump` with cleanup to the next evaluation block, forwarding every live scalar
binding. The temporary is neither retained until caller return nor represented
as another authored call. Each such source statement has exactly two captured
calls, with an ordinary or boundary producer. Earlier locals, later statements,
and successive producers in separate statements share the ordinary schedule;
each temporary dies before the next statement runs. Multiple arguments or
producers in one call, other consumer return carriers, mutable/write-only or projected result
operands, self consumers, linear result claims, and sum-payload inspection remain
separate work.
Anonymous boundary structural results use the same argument schedule as
ordinary affine producers. Static requirements, bodyless declarations, and
caller-owned nominal requirements retain their exact source target separately
from the resolved boundary declaration. Each temporary is established after
successful provider completion and either transfers exactly once to its enclosing
consumer or retains ownership through the bounded shared-read continuation above.
Provider refusal preserves completed operands for retry; a later
operand crash preserves earlier effects without a cleanup successor.

Scalar-returning boundary callers also use this evaluator for their existing
two-statement body: an immutable boundary-result initializer followed by that
local's return. Nested operand calls use shared scalar-helper identities; they
do not create synthetic Unit bodies or additional source statements. The root
retains its checked crash contract and whole-root claims while arguments execute.
Only successful boundary completion settles those claims and supplies the
returned scalar. Source validation rejoins the initializer, its captured call,
the declared result carrier, and the returned local independently of operand
graphs. The emitted root and scalar helpers retain their exact source owners.

Ordinary Unit closures also retain these boundary-return bodies as callees with
immutable scalar values and structural parameters. The body remains a boundary-return
plan, not a manufactured scalar graph. Its attachment, bodyless boundary,
service ceiling, nested scalar helpers, result, and crash contract join the same
module before identities and proofs are finalized. Repeated calls share the
same callee; identical boundary declarations from distinct checked owners
coalesce, while conflicting declarations reject. The caller keeps its own root
service reach. Nested operand calls may select further parameterized wrappers
in this shared Unit catalog. Scalar formals retain authored source positions
separately from their dense value ordinals; the returned local follows all scalar
formals. Parameter ranges and supported entry predicates use the existing checked
scalar contract and canonical conjunction lowering. Each ordinary call must prove
that conjunction against its evaluated actuals before entering the wrapper.
Missing predicates and range rows reject rather than disappear. Named-root
lowering also retains mixed scalar/structural signatures, including whole-root
claim settlement and scalar-only entry relations. Structural qualifications
remain in the structural signature, not scalar proof slots. Direct checked scalar
calls retain structural arguments and exact claim transfers, emitting the existing
`CallStructuralScalar` operation. Their source parameter identities, projection
paths, and transfer events rejoin the authored call; scalar result production
does not discard structural custody. Callee claims use machine-local dense IDs,
independent of the module-wide value and operation identity ranges.
Scalar-only computed edges still reject structural signatures, even when another
direct call has retained that same callee in the module. Previously established
affine structural results use their existing result owner and single-consumer
cleanup rules; this does not introduce linear result claims. Provider rejection
retains custody for retry, and an operand crash invokes neither the boundary nor
cleanup. Constructed local arguments reuse the existing leading empty-record
prefix or single-i64-field record establishment. The source declaration,
constructor value, and exact establishment/transfer events remain authoritative;
scalar result ordinals do not count construction locals. Only transferred locals
lose caller cleanup, and unused locals retain reverse declaration order.
The verifier shares the ordinary Unit local/result policy and still rejects
borrowed or projected locals, missing establishments, and duplicate disposal.
Nested whole affine result expressions use the same authored argument schedule
as ordinary Unit and structural calls. The wrapper's returned scalar extends
the source binding namespace only after successful completion; private staged
operands do not become locals or displace earlier bindings.
Wider construction carriers, structural-observation requirements, broader result
guarantees, and mixed structural-field crash predicates remain separate work.

Composed-control boundary leaves use the same evaluator, including the existing
three-state, prefixed, nested acyclic, dynamic-result continuation, and closed-sum
payload routes. The producer partitions exact outer flow calls from nested
operand occurrences; the outer boundary remains one operation. Private blocks
execute only within the selected leaf, carry completed scalar arguments once,
and preserve the leaf's original value namespace for subsequent boundary calls.
Whole-root linear settlement retains its claim until the boundary succeeds;
scalar operands do not change structural argument or receipt positions.
The selected scalar helper closure retains exact source targets, callee
requirements, and crash contracts, and its identities are disjoint from Unit and
dynamic-realization machines. Operation proofs are completed on the assembled
module. Closed-sum payload execution still requires interpreter case inspection;
operand support does not supply that missing runtime carrier.

Ordinary Unit-call leaves in the three-state, prefixed, and nested acyclic
control routes, dynamic-result continuations, and closed-sum payload continuations
use that evaluator too. A control-state Unit-call prefix completes before its
guard; the guard and successors use the retained source values after operand
evaluation. The callee's parameters, statements, transitive calls, providers,
and normal cleanup come
from the same complete Unit-body lowering as standalone Unit machines.
Free callees keep no attachment; a static data qualifier does not manufacture
a receiver.

Unit crash ceilings may combine Boolean and fixed-integer scalar parameters
with the supported structural member predicates. Scalar parameters use dense
primitive positions; structural predicate roots retain authored positions, which
lowering explicitly maps to the emitted structural places. The runtime signature
still uses dense structural positions. Free and hosted mixed-signature Unit
functions share the same parameter collector; free functions gain no attachment
or receiver. Ordinary calls instantiate the checked parameter-relative routes
with the exact completed scalar arguments and structural places, including equal
and reordered operands. Composed internal calls retain their scalar-only argument
restriction. The independent verifier
substitutes scalar values as well as structural places in the callee contract
and requires the exact invocation routes and caller-ceiling coverage. A true
guard permits a crash; it does not execute one. Explicit crashes in the invoked
body still preserve preceding effects and have no normal continuation.
Coverage recognizes a block parameter as a forwarded machine formal only when
every incoming scalar edge supplies that same formal with the same type.
Unknown or conflicting inputs, structural payloads, and cycles without an
established origin do not establish that equality. This affects only the
caller-ceiling comparison; the invocation still retains its actual argument IDs.
Crash-route comparisons normalize conjunction/disjunction order and duplicate
leaves after substitution, including when two formals receive one actual value.
This comparison normalization does not change the codec's canonical wire order.

Proof-gated mixed Unit crash arithmetic retains integer comparison requirements
over scalar formals, structural members, and literals. The closure allocates its
real scalar formal declarations before lowering contracts, and the machine bodies
reuse those declarations. Each ordinary call substitutes its completed scalar
arguments into the callee requirements before checking predicate totality, then
rebases structural roots to the actual places. Requirement slots keep the callee's
canonical order even if reordered or equal arguments change the substituted terms.
Each call slot becomes an obligation finalized after complete Unit or cleanup
assembly. The shared integer certificate producer consumes the independently
reconstructed pre-call requirements and facts; the callee's own guarantees cannot
prove its preconditions. The independent verifier reconstructs scalar and
structural substitution, including the exact ordering of equality operands.
Reversed equalities carry explicit symmetry certificates instead of silently
reordering the proof goal or its premise.
Pure immutable Exact arithmetic arguments retain their completed result IDs in
the call requirements. Complete supported integer entry-requirement packages
remain present even when the caller's crash ceiling is unconditional or absent.
Nominal cleanup retains ownership of its contextual caller and target contracts;
the shared Unit closure does not interpret those provisional namespaces as final
parameters.
Ordinary Unit requirements also retain Boolean parameters and exact structural
Boolean members, constants, negation, equality, conjunction, and disjunction.
Scalar and Unit contracts share bounded Boolean polarity conversion; each owner
supplies its own parameter/member namespace and integer-bound encoding. Unit
integer comparisons keep their original strict or inclusive spelling. Scalar
and structural actual substitution preserves requirement slots and child order;
reordered or shared arguments do not recanonicalize the reconstructed obligation.
These requirements use the same independently checked pre-call certificates.
Case analysis projects nested disjunctions from conjunctions with explicit
elimination proofs; it does not turn a conjunct into an uncited entry assumption.
This does not establish totality from integer bounds hidden inside logical
connectives or extend requirements to unsupported arithmetic operands.
Computed Boolean actuals retain their executed result IDs. Nonliteral Boolean
operation rows declare an equation plus positive and negative polarity
implications. Reconstruction derives each implication only from that operation's
typed denotation, using checked Boolean conversion with no caller hypotheses.
The original equation remains first; both implications follow before any later
call obligation is captured. The producer cites these implications explicitly,
proves their premises, and transports the result through exact equality proofs.
The separate private crash-path walk retains the original operation equations,
without adding these redundant auxiliary implications to its bounded path
copies. It still retains authored guarantees; ordinary operation and call proof
reconstruction independently checks the complete fact set.
Implication search is bounded to 4,096 steps and depth 64; a cycle supplies no
premise. Neither an authored call requirement nor a callee guarantee can supply
the missing operation meaning.
Source evaluation checks the selected operator meaning in the expression's
owning machine. Substituting a callee formal switches to caller ownership once;
the caller's argument does not inherit a callee specialization's operator
selection. Closed Boolean equality uses that checked meaning, not the spelling
of a selected user-defined comparison.
Bounds guards likewise recover the actual collection-length carrier and the
carrier of a supported builtin integer computation before excluding unrelated
operator candidates. Collection length requires an exact structural receiver;
a nominal field named `len` retains its declared type. Computed operands require
builtin inner-operation meaning and compatible known operand types. Anonymous
literals and unresolved operands remain independent wildcard candidates, not
copies of a sibling's type. Neither type recovery nor a visible comparison
declaration supplies a bounds proof or changes the selected operation.
Call preconditions cannot fall back to comparing an uninstantiated callee name
with a caller fact: identically named formals do not establish anything about
the actual argument.
Arithmetic formation obligations precede the call
obligation; the shared proof producer uses reconstructed operation equations and
caller facts rather than replacing the argument with its authored expression or
manufacturing an exact caller premise. Source mathematical substitution does not
erase the runtime computation or its independently verified overflow obligation.
Division nonzero and signed overflow bounds, and Exact shift count and value
bounds, remain required. This does not add mixed projected-argument partial cleanup.

The supported composed-control leaves accept either a semicolon call or a final
Unit call expression. This includes boundary leaves after dynamic scalar-result
dispatch and the final boundary call in a closed-sum payload leaf. Only the
selected leaf evaluates its operands and invokes its callee. A trailing expression
retains its exact state, statement coordinate, and captured outer call even for
pure or zero operands; it is not rewritten into a discard or another state's
call. Calls before transitions still require statement syntax. Result, signature,
structural-custody, and control-topology restrictions are unchanged.
Provider-field receivers rejoin the exact attached storage declaration through
the existing inherited-field mapping; missing or conflicting receiver stamps
cannot be recovered from their spelling during publication.

The composed root and its Unit callees select one type, boundary, service,
and helper catalog before assigning identities. Scalar helpers shared by root
operands and callee bodies have one machine identity. The root graph consumes the
shared identity counters and is inserted before proof finalization; independently
lowered modules are not concatenated or relocated. Complete Unit-body lowering
replaces the former parameterless-only composed target emitter.
The exact source-to-machine map survives dispatch, so callee suspension,
conformance, and float-source metadata are selected with their actual owners
rather than treating the composed root as the entire source closure.

Ordinary callers can also invoke composed Unit bodies.
Both checked body catalogs are pruned together to a complete call graph; a
missing transitive body removes upstream callers from either catalog. Lowering
borrows each body's actual entry signature and operations, allocates shared
machine/header identities, then emits each authored body once. Composed callees
and roots share emission and return normally to their callers; states are not
converted into synthetic machines. Nested ordinary and
composed calls share scalar helper identities and proof obligations. Whole-root
linear arguments retain their call transfer, state-entry claim aliases, and
selected boundary settlement. Calls from composed leaves still require
scalar-only targets without runtime entry requirements. Claim-free graphs with
scalar parameters and shared byte views use general state traversal
for both free and attached bodies, including branches, joins, and empty states.
Qualification, owned-frontier, implicit-receiver, and closed-sum cases retain their
specialized routes. Once the general route is selected, a failed source check
does not fall back to a shape matcher. Scalar declaration/assignment prefixes
reuse the shared local-storage namespace. Branch-free scalar successor operands
are evaluated only after their edge is selected, then passed simultaneously to
the target state. Selected-edge subslices retain their exact operation-result
descriptors through later states and repeated loop iterations, including
descriptor-selecting joins. The same route retains the supported authored
`Slice::Length` witness and its natural-ranked component evidence; see
[borrowed-byte writer composition](#borrowed-byte-writer-composition).

Closed-sum continuations retain the structural-result boundary and its exact
case/payload transfer independently of their ordinary callees. Calls within a
selected payload leaf preserve that leaf's scalar namespace for subsequent
operations. Root control blocks, values, operations, edges, and storage places
use the shared closure's identity counters. The root's provider requirements
cover its entry boundary and leaf boundaries together; callee requirements
remain owned by their complete Unit bodies.

Direct, rebound, stored, and forwarded dynamic-result continuations retain that
same ordinary call closure. Shared Unit and scalar identities are assigned
before dynamic realizations and forwarding helpers; already-emitted calls are
not renamed. The caller's blocks, values, places, edges, and descriptor
establishment operation use the shared counters. Dynamic root conformance rows
remain separate from selected callee applications, with exact source owners
retained for the whole published module. Only the selected leaf runs its operand
computations and ordinary body. An unused provider-backed field remains in the
root attachment without inventing a direct boundary requirement from a callee's
independent call. Producer, codec, and verifier each require exact equality
between the root's actual direct boundary calls and its attachment requirements;
neither transitive reach nor a retained field supplies a missing requirement.

Structural returned calls still need connections to the shared evaluator.
Structural arguments on composed internal calls,
mixed runtime requirement proofs for call-bearing arguments, other arithmetic
policies, and mutable snapshots,
and caller-ceiling proofs for computed-argument
routes remain implementation work.
Existing control-state signature restrictions remain; operand evaluation does
not itself add general scalar state-argument transport or computed dispatch guards.

Computed Boolean guards complete before either branch destination starts. A
private dispatch block consumes the completed Boolean and retains the source
value prefix for the selected branch. Guard roots rejoin the exact authored
`When` expression and Boolean carrier. A single-subject Boolean dispatch with
exactly opposite literal arms tests the subject once and treats its final arm
as fall-through, in either authored arm order. Independent anonymous guards are
not combined merely because their call expressions look alike. Longer computed
dispatches still need explicit shared-subject planning.
Existing lone-call comparison hoists still produce a direct-call binding and
pure guard; retained nested or multi-call guards use computation roots.
Publication also checks the authored fallback: a later independent `When`
cannot reuse an unconditional edge from an earlier checked dispatch.
Fall-through proof contexts retain failed earlier guards, including their
Boolean polarity. Inductive contract and ranking judgments use those facts
for the exact selected edge; range propagation still invalidates facts affected
by guard effects or intervening writes.

An authored standalone `crash Trap;` or `crash Abort;` may be the fallback
after a guarded scalar return or state transfer. The checked destination keeps
the exact statement coordinate; lowering rejoins its authored cause, live
terminal target, absence of a continuation, and checked crash-site evidence.
Only the selected fallback enters a private block with the existing no-successor
crash terminator and explicit frontier lower bound. A computed guard completes
first, so its own crash precedes the fallback and a successful ordinary arm
never executes the crash. This adds no inline crash-arm syntax or new Terminal
operation.

Crash-route coverage includes stable consequences of earlier failed guards in
the same source state. These predicates refer to exact immutable entry-parameter
symbols, not same-spelled locals or parameters of another state. Boolean
projection can retain `!flag` from a failed `flag || effect()` guard without
treating the call result as a parameter fact. Unrelated local writes cannot
invalidate the retained entry snapshots; mutable-storage route transport
requires its own value evidence.

Immutable and mutable initializers and local assignments in free scalar machines
use that same evaluation path. Each RHS completes before the following statement;
private continuation blocks carry prior values and the new result. A computed
assignment reads the old storage value throughout its RHS and updates the current
storage position only after that value completes, without changing earlier
immutable snapshots. Assignment roots rejoin the exact authored RHS and prior
mutable local destination, including its declared carrier. Initializer roots are
checked against the authored local's initializer, carrier, mutability, and exact
destination identity or immutable binding ordinal. Pure initializers and simple
immutable calls with pure arguments retain their existing flat binding path.
Exact-cast facts for assignment right-hand sides are collected against the
pre-write value environment, before invalidating the destination's old facts.

Owned mutable Boolean and fixed-integer parameters in free scalar machines
use the same current-storage mapping. Checked state graphs retain arena-backed
entry rows naming the authored parameter ordinal, symbol, and carrier. Lowering
matches each row to that exact state's mutable declaration before initializing
storage from the incoming operand. Immutable peers keep their authored ordinal
slots, but a body read of a mutable formal cannot use an immutable entry alias.
Each transition initializes its target state's storage from the delivered
values; no entry-machine or sibling-state binding is recovered by spelling.
Copies established before reassignment remain separate values. A postcondition
mentioning a mutable formal still describes its final value, not an implicit
snapshot of the caller's earlier argument. Declared requirements and published
crash conditions are separate entry contracts: their checked predicates retain
incoming parameter values, while executable guards read current storage.
An incoming edge predicate over mutable owned scalar storage is not evidence
about an invocation-entry crash route. Direct-site and call-route coverage
exclude that predicate until its entry-value origin is retained independently.
Checked coverage separately uses exact Boolean machine/entry-state requirements
as invocation-snapshot hypotheses. Named-state requirements do not become
ambient entry facts. Requirement names must agree both with their exact formal
symbols and with the canonical entry ordinal; body reassignment does not change
those hypotheses. All-crash scalar graphs retain their checked requirements
instead of dropping the contract because no normal return exists.
The strict Boolean-formal entry reader also accepts attached and mixed
signatures. It rejoins an attachment through one live nominal symbol and checks
its retained name; machine and owner generic/lifetime binders remain excluded.
The entry state and every parameter must retain their exact live symbol owner,
with no duplicate parameter symbols or names. Each observed leaf is one exact
non-const, non-receiver owned Boolean formal, not a field or borrowed Boolean.
Unread structural operands and receivers retain authored positions for crash
identities; checked scalar operands count only preceding primitive parameters.
Mutable formal reads in this contract namespace rebind to that scalar entry
position, independently of the execution support for the enclosing body.

Independent call-ceiling validation proves the union of a same-cause bucket's
published alternatives from caller requirements. It reuses the checked Boolean
predicate-denotation conversion and certificate search used for direct sites,
but supplies only entry requirements, never CFG facts or current body values.
One union goal shares the 4,096-step conversion and search limits across the
bucket; proof depth is bounded to 64. Exact callee continuations remain unchanged.
This also permits a disjunctive entry requirement to cover separate published
alternatives without claiming either alternative holds by itself.
Common consequences of nested alternatives use kernel-checked disjunction
elimination: each ordered branch proves the same goal with only its exact
alternative added as a local assumption. The assumption is discharged before
the next branch. Conjunction projections and case branches share the existing
search budget and depth limit; semantic axioms never occupy requirement slots.

Checked source coverage recognizes equivalent negation and Boolean-literal
equality wrappers only after verifying builtin meaning and the exact entry
formal/symbol and canonical ordinal on both requirements and authored routes.
It retains the original published predicate identity as a proved consequence.
Strict entry-only consequence extraction unions conjunction facts and intersects
facts from every disjunct, preserving polarity through negation and literal
Boolean equality. Published compound routes must be established completely from
those retained facts. The shared runtime-guard reader is unchanged; current body
storage and selected authored operators cannot supply entry hypotheses.
Common-consequence intersection and route assembly share 4,096 work units and
depth 64; exhausted work is not retried with a fresh budget for another route.
Negated scalar conjunctions and disjunctions lower through logical De Morgan
propositions, including Boolean-literal equality wrappers, not eager scalar
operations. Scalar and structural crash predicates also admit equality between
compound Boolean predicates: equality combines the two equal-polarity branches,
and inequality combines the two opposite-polarity branches. Contract and crash
lowering share that logical constructor while retaining their separate atomic
denotations and namespaces. The whole crash predicate has one 4,096-unit logical
input/expansion budget and depth 64, including all branches and top-level
conjuncts. Unnormalized scalar constant connective children still reject; atomic
Boolean and numeric crash encodings remain unchanged. Structural composition
retains its existing constant-child handling and atom-specific field, IEEE,
byte-sequence, sum-equality, and case-membership readers. Equality can combine
those proposition-only leaves without inventing scalar denotations. Existing
negation of a proposition-only atom remains implication to falsehood, not a
complemented numeric comparison. Every expanded operand retains the original
runtime requirements and structural root/path checks. This logical budget does
not replace arithmetic-subtree, structural-path, or sum-case checks.

Boolean implication beyond structural common consequences, exact entry crash
hypotheses over structural predicates, and numeric entry coverage remain
implementation work.

Direct crash-site validation independently proves every asserted guard from
invocation-entry requirements and facts reconstructed before that terminator.
The private reconstruction retains raw branch-value polarity and bounded path
alternatives through CFG joins, including later selection on computed Boolean
block parameters. Every path must establish the guard or a kernel-checked
contradiction; site guards and producer evidence never become premises.
Each machine's traversal has independent limits of 4,096 block visits and
4,096 generated or copied facts; exhausting either rejects without dropping
unvisited paths.
Bounded certificate search uses exact assumptions, semantic axioms,
conjunctions, disjunction introduction and elimination, and equality rules.
The proof owner checks both the certificate and fixed Boolean/integer
predicate-denotation conversions; these private conversions add no serialized
proof rule or obligation identity.
Crash-only raw branch facts stay out of ordinary obligation reconstruction.

Ranked-site proofs use only entry requirements until all-path invariant custody
is available. Unversioned structural observations need exact shared-borrow
parameter roots to supply entry facts; current reads on owned or mutable roots
cannot impersonate their entry snapshots. Their raw scalar read values still
support executable branch predicates. Broader mutable-origin transport remains
implementation work, not a language-design decision.

Resolver operand preprocessing cannot move indexed reads or cast-wrapped calls
out of guarded transition targets or selective Boolean right operands. These
operands remain behind their original selection boundary for checked lowering.
Ordinary returns, initializers, and assignment right-hand sides also retain
cast-wrapped free calls in operand order: a later cast cannot hoist its call ahead
of an earlier operand. Assignment target/indexed-read normalization is unchanged;
computed projected writes still need their own destination plans.

Computed call arguments bind the pinned callee's parameter-relative crash
routes to the actual argument values, using the same route substitution as
ordinary staged calls. The checker row still supplies exact target-contract
identity; Terminal verification independently reconstructs route coverage.
Declared crash ceilings may remain even when every crashing branch is skipped.
Direct scalar calls, including literal arguments, bind those same pinned callee
routes to the emitted argument values. Source-side simplification of a caller's
crash conditions is proof information; it does not replace the callee's published
route interface in a call operation.

Integer applications share operation construction with pure checked expressions.
Each operand retains its own arithmetic policy; a resolved callee's declared
result type supplies its carrier and policy, not the enclosing destination.
Comparison normalization may reverse completed values for `>` or `>=` but never
reverses their evaluation. Widening and occurrence-proved exact casts use the
same pure templates after evaluating their operand once. Cast range obligations
must still be reconstructed by independent Terminal verification.

Fixed-integer result/literal postconditions, including conjunctions and
disjunctions, retain predicates over the actual Terminal machine result.
Normal-return bounds can discharge nested exact casts. Source validation joins
the reserved result occurrence to its exact contract owner and admits only
builtin numeric comparisons; a parameter named `result` is not the returned
value. Strict integer literal bounds normalize to equivalent inclusive bounds
only when the adjacent endpoint is representable. Publication reconstructs both
callee postcondition and caller cast obligations, with certificates over the
emitted return equations and instantiated call guarantees. A literal tautology
does not stand in for a result guarantee, and a changed return or weakened
guarantee cannot reuse its serialized evidence.

Integer contract predicates also name exact immutable entry parameters. Their
checked positions follow the complete scalar parameter order, with the result
slot present only in postconditions. Explicit requirements and enforced literal
parameter ranges become one canonical conjunction; calls substitute the actual
evaluated argument values and prove that requirement before assuming a result
guarantee. Equality operand order comes from the codec, with explicit symmetry
evidence when proof traversal needs the other direction. Computed argument bounds
cross a returned-value equality or order relation through checked substitution
or transitivity, including case analysis over disjoined guarantees. Named-state
forwarding retains exact immutable entry origins and rejects ambiguous joins,
including backedges to the entry state spelled through its state or machine name.

Free scalar requirements also retain Boolean entry predicates, including
negation, equality, conjunction, and disjunction. Mutable formals use their
incoming values, not later body storage. The scalar fallback requires exact
entry-parameter symbols and builtin operators; spelling cannot recover a
missing or foreign operand identity. Boolean polarity lowers into logical
propositions over the exact entry operands; literal wrappers and negation do
not require evaluating a separate Boolean expression inside call proof search.
The existing literal contract encoding and mutable-formal postcondition fence
remain unchanged. Source calls prove these requirements before executing the
callee; this does not add runtime requirement checks to host-supplied interpreter
entry arguments. Nested body-local and mutable-storage operands cannot alias
entry namespace positions in a retained contract predicate.
Compound equality expansion has a 4,096-step per-clause lowering budget;
exhaustion rejects before allocating an unbounded proposition tree.

Remaining numeric policies and selected operator calls, longer computed dispatches,
borrowed/projected operands and writes, and named runtime proof outputs still
need execution-plan extensions. Nonliteral contract arithmetic and result bounds
that need caller-specific snapshots beyond immutable scalar formal comparisons
still need complete transport. Source interval projection uses only immutable
formal declarations and builtin required bounds, never a reread of caller
storage. A call's carrier alone never proves a partial conversion.
The older flat guarded-argument and shared match-subject normalization remains
on uncovered paths and must be retired as those paths move to checked computation
planning; this work remains in `STATE-LOCAL-VALUE-FRONTIER`.

Structural-operand Unit composition is a distinct checked operation, not a
widened scalar `Call`. One free or hosted Unit body may bind a primitive result
from one selected operator whose structural operands are an exact permutation
of its whole, claim-free owned affine parameters and whose remaining operands
are checked scalar expressions. Checked planning rejoins the authored use,
strong provider plan, exact specialized realization, contract, result, and
empty service reach. Terminal production emits that realization as a separate
machine and the use as `CallStructuralScalar`; scalar values and source-ordered
structural roots remain separate argument rows, while consumed roots are absent
from the Unit return discard list. Production rechecks the same custody and
rejects missing, mistyped, unavailable, duplicate, reordered, projected,
substituted, or borrowed operands. The Unit and realization share one exact
structural-type catalog. The hosted fixed-width integer/empty affine-record
subset continues through native, object, image, and installation custody on
Linux x86-64 and AArch64. Structural results, claims, content evidence,
services, projections, borrows, nontrivial layouts, and wider control flow
remain later slices.

A nonempty path to a relevant Boolean field of a record parameter retains every
canonical structural-field identity and rebases across structural Unit calls.
For a field- or literal-fixed-index-projected structural argument, the caller's
canonical argument path is prepended to the callee's parameter-relative Boolean
path. Canonical predicate segments distinguish verifier-owned field identities
from exact array indices. The verifier independently traverses both declared
record and fixed-array paths, requires in-bounds indices, structural
intermediates, and a Boolean leaf, and rejects absent, erased, truncated,
mistyped, out-of-bounds, or redirected paths. Built-in Boolean equality,
inequality, negation, and conjunction may compose multiple relevant member
paths and literals; every nested path is independently traversed and rebased.
Equality, inequality, and ordered comparisons also accept same-typed relevant
fixed-integer member paths; terminal terms retain both the canonical path and
the exact integer type, and the verifier checks that annotation against the
declared leaf. Built-in fixed-integer `&`, `|`, `^`, and `~` compose the same
typed member terms without an arithmetic proof obligation; overloaded forms and
the distinct address carrier remain outside this bounded structural slice.
Checked production applies that address fence both to direct member predicates
and to whole-record leaf expansion; the source contract may remain in checked
identity, but Terminal lowering receives no portable scalar term and rejects it.
Whole-root and all-field-projected structural calls reconstruct those predicates
across the callee boundary by prepending the caller's canonical argument path to
every callee-relative integer-member path, including operands nested beneath
bitwise terms. The verifier repeats that substitution independently and rejects
a redirected continuation even when the redirected path reaches another valid
same-typed leaf. A built-in Boolean
disjunction retains two distinct canonically ordered proposition branches; each
branch may contain the same accepted Boolean or integer-member predicate forms.
Production and independent verification recursively rebase every branch across
whole-root and all-field-projected calls. Both codecs retain the proposition;
the semantic codec rejects nested, duplicate, or noncanonically ordered
disjunction rows. Whole-record equality does not add an opaque aggregate term:
for two same-typed `Equatable` parameters, checked production retains the
language-defined inline field expansion. A finite nonempty tree containing only
relevant Boolean, fixed-integer, IEEE `f32`/`f64`, and supported byte-sequence
leaves becomes one flat canonical conjunction. Float leaves use an atomic
format-annotated IEEE `==`
proposition rather than mathematical `Equal`, preserving NaN non-reflexivity and
signed-zero equality. Direct float-field `!=` uses the same atomic proposition
with an explicit comparison kind, preserving the complementary IEEE result
without a second verifier family. Whole-record float `!=` reuses the canonical
equality conjunction as the premise of `P -> Falsehood`, so aggregate float
negation adds neither a duplicate leaf family nor De Morgan permutations. Each
leaf keeps its left and right parameter root;
call verification independently substitutes both roots and rejects redirecting
either operand even when the replacement path is otherwise valid and
same-typed; float leaves additionally require the exact declared format.
Byte-sequence leaves use a separate atomic content-equality proposition over
two nonempty structural paths. Terminal structural identity distinguishes a
borrowed view from bounded owned storage and retains the bounded carrier's exact
capacity, but does not expose a native pointer/length descriptor. Equality is
defined only by equal live lengths and equal live byte prefixes: pointer
identity, capacity, and bytes beyond the live length are irrelevant. The
verifier independently requires both resolved leaves to have byte-sequence
carrier types, and call substitution rebases both roots. The bounded slice
admits field-to-field whole-record equality for `&[u8] in Domain` and
`[u8; N] in Domain`; text literals and direct text `!=` remain fenced. The
current semantic codec, proof-bundle codec, and installation record encode this
vocabulary. A genuinely zero-member record instead normalizes equality to the
existing Boolean `true` term; inequality uses the existing negation, and calls,
codecs, verification, fixed fuel, and interpretation reuse that carrier. An
all-erased record is not empty and remains fenced. Payload-less sums retain
their closed case roster as exact Terminal structural case identities. Equality
is the canonical flat conjunction of both case-membership implications for
each case; inequality is that complete equality proposition implying
falsehood. The verifier resolves each subject and case independently.
Payload-bearing pure sums additionally retain each exact case-payload field.
For direct relevant Boolean, fixed-integer, IEEE, and byte-sequence payload
leaves, equality is a canonical disjunction whose arm for each case conjoins
membership of both roots in that case with the exact payload-leaf equalities;
inequality is that complete disjunction implying falsehood. A case path uses an
exact case identity followed by its exact payload-field identity, and the
verifier and codecs reject unknown or redirected identities. One relevant
acyclic record or pure-sum tree directly held by a case-payload field also
expands its supported leaves transitively. Those paths retain every exact
alternating case, payload field, enclosing record field, and leaf identity in
order through nested sums, and whole-root calls independently rebase both
operands. Direct whole-root mixed shapes retain both common fields and a closed
case roster. Their equality is one canonical conjunction: supported
common-field leaf equalities in declaration order followed by one
source-ordered disjunction whose arms contain matching membership for both
roots and the selected case's supported payload-leaf equalities. Inequality is
that complete equality proposition implying falsehood. Whole-root Unit calls
independently rebase both operands, while codec format 33 / vocabulary 35,
verifier, fixed-fuel, interpreter, and installation format 40 preserve and
replay the exact common-field, case, and payload-field identities. One bounded
nested form is also included: a whole-root acyclic record may contain exactly
one relevant direct field whose type is the existing mixed shape. Every mixed
common-field, case-membership, and payload-leaf path retains that enclosing
field as its first segment, and whole-root Unit-call rebasing preserves it on
both operands. No new proposition or format is added; independent structural-
path replay rejects a substituted enclosing field. The next bounded form
admits exactly two enclosing relevant record fields. Every path carries both
field identities before the sole mixed
occurrence; equality, inequality, whole-root Unit-call rebasing, codecs,
verification, fixed fuel, and interpretation replay that exact ordered chain,
and mutation of either field rejects independently. The following bounded form
admits exactly three enclosing relevant record fields. Equality, inequality,
whole-root Unit-call rebasing, codecs, verification, fixed fuel, and
interpretation retain all three exact field identities before the sole mixed
occurrence, and mutation of any prefix rejects independently. A fourth bounded
form admits exactly four enclosing relevant record fields and replays the same
complete ordered path through equality, inequality, whole-root call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A fifth bounded form admits exactly five enclosing relevant record
fields and replays the same complete ordered path. A sixth bounded form admits
exactly six enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A seventh bounded form admits exactly seven enclosing relevant record
fields and replays the same complete ordered path through whole-root equality,
inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. An eighth bounded form admits
exactly eight enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A ninth bounded form admits exactly nine enclosing relevant record
fields and replays the same complete ordered path through whole-root equality,
inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A tenth bounded form admits
exactly ten enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. An eleventh bounded form admits exactly eleven enclosing relevant
record fields and replays the same complete ordered path through whole-root
equality, inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A twelfth bounded form admits
exactly twelve enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A thirteenth bounded form admits exactly thirteen enclosing relevant
record fields and replays the same complete ordered path through whole-root
equality, inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A fourteenth bounded form
admits exactly fourteen enclosing relevant record fields and replays the same
complete ordered path through whole-root equality, inequality, Unit-call
rebasing, codecs, verification, fixed fuel, interpretation, and independent
prefix mutation. Fifteen or more enclosing fields, mixed values below case
payloads or another mixed shape, two mixed sibling fields, direct projected
mixed comparisons, recursive cycles, address and erased payload equality,
written `equals` bodies, and runtime sum layout remain outside this bounded
terminal slice. When an acyclic relevant record field reaches a payload-bearing
sum, the same sum proposition is retained below that field path, and independent
verification preserves the complete `Field -> Case -> Field` identity chain.
Direct source-call rebasing
through a sum-bearing projection remains fenced with runtime sum projection and
cleanup.
Arithmetic over
same-typed relevant fixed-integer members accepts Exact addition, subtraction,
and multiplication: each member or fixed-integer-literal operand retains its
exact checked carrier, nested operations remain typed `ExactIntegerAdd`,
`ExactIntegerSubtract`, or `ExactIntegerMultiply` terms, and whole-root or
all-field-projected calls rebase every member leaf recursively. The verifier
independently repeats that substitution and validates every declared leaf and
arithmetic-node type; both codecs preserve the nested term. Policy-selected
fixed-integer members also accept the total Wrapping and Saturating forms of
addition, subtraction, and multiplication. The terminal term retains the exact
selected behavior, and projected calls, codecs, verification, fixed fuel, and
interpretation preserve it without an overflow obligation. Wrapping left and
right shifts are likewise retained as total structural terms: the value's
carrier and the independently typed integer count remain distinct, and the
language-defined Euclidean count reduction survives projected calls, codecs,
verification, fixed fuel, and interpretation without a count obligation. Exact
right shifts accept a self-proving in-range literal count or a complete retained
package proving a runtime count nonnegative and below the shifted carrier width.
Exact left shifts require the same count evidence plus carrier-tight value bounds
at the greatest possible count; a zero count or a compile-known value that shifts
safely is self-proving. The producer canonically orders the complete requirement
package, and projected calls rebase one exact obligation per requirement.
Independent verification reconstructs the count and overflow checks and rejects
missing or weakened evidence. Direct Trapping arithmetic remains forbidden in
predicate terms. An explicit fixed-integer or address `embed` instead lowers to
an unbounded proof-`Int` term carrying the source carrier identity and exact
derived range; an explicit same-carrier `as` lowers to Exact arithmetic and
retains its discharged representability obligations. Wrapping and Saturating
predicate nodes retain their distinct total denotations. Terminal Psi never
creates a proof-side Trapping node or a predicate-generated crash effect.
Executable Trapping operations separately retain their compiler-owned
primitive trap predicate and path-conditioned crash site. Verification checks
that denotation against the primitive catalog and proves the derived guard is
covered by the authored same-cause route disjunction.
Exact division and remainder accept a
same-carrier literal divisor only when it is nonzero and cannot trigger signed
`MIN / -1` overflow. Wrapping and Saturating division and remainder accept any
same-carrier nonzero literal, including signed `-1`: their selected policy
defines the `MIN / -1` result, while division by zero remains illegal.
A whole-root structural Unit closure may instead name a runtime integer-member
divisor. For Exact operations, each machine's complete bounded `requires`
package must prove one of the verifier-owned totality shapes: `1 <= divisor`,
`divisor <= -2`, or the joint signed bounds `divisor <= -1` and
`MIN + 1 <= dividend`. For Wrapping or Saturating operations the corresponding
package need only prove the divisor nonzero through `1 <= divisor`,
`divisor <= -2`, or `divisor <= -1`. Checked plans retain those packages without
source handles and terminal Psi publishes the exact requirements. Every direct
or all-field-projected structural call carries one exact obligation per callee
requirement; the producer rebases the target place through the caller's
canonical field prefix, cites the matching caller assumption, and emits a
replaceable certificate. Independent verification reconstructs that prefix,
repeats the rebasing, and checks the assumption index before codec or
interpretation. Removing evidence or weakening or redirecting a bound rejects.
Case-payload paths and imported crash capsules remain fail-closed.
Structural/content contracts reject because custody effects require their own
vertical slice rather than an ordinary scalar flag.

The interpreter uses owned call frames and charges the call before entering the
callee. Sponsor exhaustion in the callee resumes without replaying that paid
call. A callee crash escapes as the original no-successor crash site and uses
that callee edge's fuel charge; call composition records the surviving route
without fabricating or double-charging another executable crash. Validation
rejects recursive call graphs until terminal Psi can carry and verify the
required tail-position and ranking evidence. Fixed-fuel derivation includes
separate acyclic callee return/crash bounds: caller tails compose only with
normal returns, while callee crash paths terminate at their own edge. It retains
its own cycle rejection as defense in depth.

Omega selects each callee's native calling plan and evaluates arguments into
disjoint frame spills before filling their ABI homes. Assignment retains
explicit register or outgoing-stack destinations. Emission materializes the
complete outgoing area, including Microsoft x64 shadow space, preserves x86
call alignment and the AArch64 link register, and emits a typed internal-call
relocation tied to the exact Psi operation and callee. Conditional-control
emission preserves live entry registers across condition calls and rebases
relocations from independently encoded conditions and arms into final function
order.

An unconditional crash continuation requires no caller-side machine-code
branch: the verified internal call reaches the emitted callee crash leaf and
cannot return along that execution. Omega still resolves the typed call
relocation and preserves the callee leaf; it does not reinterpret a crash as a
scalar result.


### Content-conservation proposition slice

The content slice extends structural-place terms with an entry/current version;
it does not add a general historical-expression modality. It carries the exact
owner-unique content-projection identity, canonical
`IntervalSet<CoordinateSpace>` and `CountedQuantity<Unit>` terms, variadic
partial `separate(...)`, containment and equality, and canonical interval-set
residual difference. Sealed claim-frontier rows record content introduced into
or transferred out of checked custody.

The verifier infers identity-preserving reshuffles. Each canonical partition-
composition row names the exact call operation that produced it. Validation
replays the exact substitution, requires that call's internal callee contract
or bodyless boundary declaration to publish the alpha-equivalent authored
conservation guarantee, and checks the call's structural arguments against the
recorded substitution. Reconstruction introduces the derived theorem only
after that exact call completes successfully; it is unavailable to earlier
operations and absent from rejection or crash paths. Boundary guarantees are
canonical semantic rows, not provider admissions. Report fingerprints identify
canonical content for diagnostics and caches; neither a producer-carried row
nor a matching compact value authorizes a theorem by itself. Structural-result-rooted
correspondence is admitted only through the explicit root-only internal
`CallStructural` carrier, retaining the explicit returned-claim mapping in the
[call contract](../../../spec/terminal-psi/calls_and_outcomes.md#normal-results).
Wider or bodyless structural results
remain fenced. At a bodyless partial
boundary, Psi derives the kept content and residual and permits the provider to
admit only acceptance of custody for that exact residual—not the partition
arithmetic. External root correspondence and fresh issuance remain scoped
admitted hypotheses with provenance; downstream conservation remains derived.

The whole-claim source slice closes one native custody-exit path without
weakening that rule. A verified qualified linear structural input may enter a
scalar native function only when the same boundary call retains its exact
whole argument path, entry claim source, and completion receipt. Qualification
does not change the ABI carrier shape; it remains in the zero-runtime
completion-custody catalog. The selected admitted provider, target operation,
machine settlement, object, image, and canonical installation all replay that
same catalog, and source, receipt, or provider substitution rejects before
publication. This is custody transfer to a selected provider, not authority to
mint a replacement claim.

Program-local root introduction is a separate derived origin, not an admission
and not a producer-authored aggregate. Terminal Psi retains one canonical
introduction schema for each exact domain-authorized requirement parameter
that may be installed as a fresh root: requirement and subject-position identity,
qualification, owner-unique content projection and algebra, normalized exact
capacity or constrained-family instance, and artifact/lifecycle scope. The
portable verifier reconstructs those fields from the semantic module and
rejects a result route, ordinary call, missing parent lineage, unbounded
capacity, or non-enumerable installation shape masquerading as introduction.
The schema does not define its own denominator: its projection, algebra, and
capacity expression must equal the independent normalized definition retained
on the qualification owner.

The concrete installation record supplies the selected satisfier, exact slot
occurrences, lineages, finite cardinality, and epoch. Installation verification
joins it to the reconstructed schemas and derives the aggregate content demand
for one installed artifact instance; it never accepts a producer-authored
aggregate. Cathedral composes verified instance totals across the live
component/era set and charges coexistence at peak. A parent root enforces a
shared cap only within its own installed assembly and epoch, while a
cross-epoch ceiling requires authority preserved across the epoch boundary.

The Rust product implementation performs that join through the canonical installed-root
ledger. It retains the exact target-required slot closure, issues one cohort
verifier, restricts prebinding to those members, and commits the complete
eligible set atomically for one lifecycle ledger and epoch. The closed aggregate
is an ordered roster of exact occurrence identities, algebras, and
per-occurrence expressions plus derived cardinality; compact identities remain
report keys and no scalar multiplication is inferred for interval or
subject-dependent content.

The cohort and its runtime may project a cloneable aggregate snapshot for
reporting. The snapshot privately retains the exact required-slot closure and
cohort identity even when its aggregate row set is empty. Coexistence
composition compares those snapshots with the authoritative live-era roster
and accepts exactly one per live epoch. It preserves rows and epoch attribution
instead of reducing unlike algebras or symbolic capacities; the report is an
input to deployment policy, never minting or lifecycle authority.

The closed cohort then becomes a non-clonable epoch runtime, not a vector of
mint grants. A generated installed-entry subject binds the exact root, ABI and
semantic parameter positions, qualification, carrier, invocation, and runtime
place. Runtime establishment checks that subject against one dormant cohort
member, requires exactly the scalar paths named by the portable expression,
evaluates proof-natural arithmetic into a canonical content value, and commits
the fresh account only after the lifecycle lease is still current. Failed
evaluation or substitution returns the subject without removing the member.
The resulting account retains the full installed occurrence and lease; its
lineage ID is only a report key. Aggregate schemas remain accounting evidence
and are not silently converted into one shared parent root.

The same installation registry owns build-bound progress closure. It seals the
complete selected provider-plan set and its domain-separated selected-closure
digest to exact installed provider occurrences,
then admits a `ProgressProfile` receipt only when the exact issuer occurrence
realizes one owner-authorized boundary route and the receipt qualifies the
exact subject occurrence. Issuer and subject need not be the same occurrence.
Component closure checks every canonical pending row and replays the manifest's
own domain-separated digest before committing. A compact-equal manifest or
selected-provider closure with different exact structure rejects. The compact
values are compatibility-report coordinates only. Closure retains the original
manifest plus exact evidence. Terminal installation
format 42 records structural access modes plus the manifest and acceptance
report identities in the hashed installation bytes. Runnable publication
additionally joins the complete
terminal object and image, canonical installation record, the linear
`InstalledCode` claim itself, and opaque acceptance, then retains that
non-forgeable carrier for the live component era. Failed binding, publication,
or retirement returns the exact custody unchanged; successful retirement is
the only operation that releases it. The production deployment owner now
consumes the compiler candidate and real installed code, validates exact bytes
before the one-shot registry claim, and retains a retryable staged session
through provider closure, progress closure, canonical installation replay,
and runnable binding. Installation identity retains the complete selected-plan
set, including selected plans with no execution in this image. Runnable binding
compares that set with the sealed registry even without progress and retains
the registry until era retirement; only the retired carrier exposes its parts.
This composer rejects installed external-root records until their code-borrowing
handles gain an owned teardown protocol. A source-derived progress-free canary
exercises this path from Terminal-Psi lowering through component-era
publication. A selected-entry progress-bearing canary now does the same with
one exact source-derived `self.field` premise, source-selected provider plan,
installed provider occurrence, authorized establishment route, and opaque
acceptance; malformed closure input returns the claimed session for retry.

External-root execution summaries do not become authority while crossing this
installation boundary. Compact normalized-root, provider-execution,
opaque-exit, stack, fuel, boundary-contract, and selected-closure values are
named report coordinates. The ledger retains the exact validated root,
boundary, resource columns, provider exit assurance, and installed occurrence,
and its public record also carries the strong selected-provider-closure digest.
Compact-equal root-policy substitution therefore fails exact admission replay.

Psi now owns one canonical source-free handoff carrier. The
`CanonicalTerminalArtifact` retains exact semantic, proof, optional debug, and
manifest identity bytes; construction independently decodes every section and
rebuilds the manifest before custody crosses into Omega. The typed compiler
`TerminalArtifact` product then re-decodes the canonical semantic and proof
sections and runs the Terminal verifier under the request's exact admission
profile before returning the carrier. It stops without entering StateGraph,
native emission, output, or installation. Unsupported or unproved Terminal
vocabulary rejects there and cannot select another backend as a fallback.

Target-owned callback placement remains outside that source-free vocabulary.
The checked-to-Terminal producer therefore treats the complete ordered sidecar
as opaque by-value custody and returns it unchanged beside either the artifact
or a production rejection. Omega retains the resulting artifact and sidecar as
one compiler product with no consuming artifact-only projection. The private
driver route supplies checked provenance and order; the retained-product
validator replays the artifact and every row but does not infer their origin.
The canonical native-realization seam has a separate generic adapter that
returns the opaque sidecar by value beside either the successful native
artifact or diagnostic rejection. It neither adds callback vocabulary to the
source-free artifact nor interprets the rows. Distinctly, the compiler's
source-evaluated-import re-entry now interprets one direct callback row and
consumes it through target lowering and assignment before the explicit emission
fence. Neither route publishes callback custody into `NativeArtifact`. This
establishes custody only, not registration, invocation, address, lifetime, or
publication authority.

The compiler exposes one production native-realization boundary after that
owner. It consumes the complete canonical artifact by value, then performs
portable verification, target assignment, machine emission, object/image
construction, and exact final-image replay into a non-visible
`NativeArtifact`. Its production signature accepts no checked, typed,
syntax, or source representation and cannot lower or re-encode Terminal Psi.
The ordinary retained-native product stops at this carrier. Component staging
wraps the same carrier with the richer source-selected provider-plan facts and
any nonempty component-progress manifest; it is not a second lowering path.
Target substitution, unresolved or duplicate boundary settlements, provider
executions outside the exact selected requirement closure, and object/image
replay drift reject before either carrier exists. A direct native request also
rejects pending component progress rather than discarding it. These carriers
deliberately have no output path, visibility receipt, installed provider
occurrence, progress-establishment receipt, or `InstalledCode` claim:
compilation assembles evidence but cannot mint runtime or publication
authority. The deployment owner consumes the component candidate, joins
real installation occurrences and receipts under the live registry, acquires
installed-code custody, and only then produces a runnable carrier.

An authority-distinct requested-native entrance now extends that same
source-free owner without widening `NativeArtifact`. A direct request returns
exactly `NativeArtifact`; an import-bearing Linux object with an exact
normalized interpreter returns a separate, non-installable
`DynamicElfNativeArtifact`. That carrier retains the complete canonical
Terminal artifact, object, selected provider closure and executions, Terminal
authority policies/review, D29 coverage, D32 physical evidence, and requested
dynamic image. Replay rederives the image route from the exact object and
rejects PSI-only or outer-object substitution. Rejection returns the exact
image request. No admission, loader-policy, installation, publication, or
execution authority is created.

The source-free native carrier belongs to the neutral
`native-artifact` crate. The component-specific wrapper remains
in `component-candidate`; compiler and deployment depend on
those neutral owners without a cycle. The native carrier labels the retained
selected-provider `u64` as a compatibility report coordinate and separately
retains a domain-separated SHA-256 commitment to the complete exact selected
closure. The component wrapper independently recomputes that commitment before
accepting the source-policy rejoin, so a compact-equal structural substitute
cannot cross the handoff. Constructing either carrier grants no
authority: deployment still independently replays the artifact, installation,
provider, and progress joins before any registry claim or publication. The
compiler output owner now accepts a
deployment-finalized runnable at one explicit terminal-output seam. It derives
the canonical build-directory destination from the image's sealed filename and
delegates consuming publication to deployment; it does not accept selected
plans or compiler TCB authorization as substitutes for installation custody.
Every rejection returns the exact runnable and derived path. The deployment
owner consumes the finalized runnable to publish one flat executable: it
replays the canonical installation/image join, stages and validates the exact
sealed bytes and executable mode before atomic rename, replays the visible
file, and returns a non-clonable installation/image/path receipt while retaining
the runnable. Every failure returns the exact runnable and requested path for
retry; receipt replay detects later byte or mode drift.

The compiler now also exposes one typed transaction above those deployment
stages. Its input owns the candidate, real installed-code occurrence, exact
provider-occurrence bindings, exact progress attestations, and profile
decision. It consumes them in order through begin, provider closure, progress
closure, finalization, and flat output. Rejection is stage-typed and retains the
exact current deployment carrier plus every unconsumed later input; no path
converts a linear failure into diagnostics alone. Progress-free rejection and
progress-bearing success canaries cross this transaction. `CompileReport` can
retain the complete non-clonable result as a publication lane mutually
exclusive with legacy executable receipts. It replays the
deployment before taking ownership; rejection returns that deployment intact,
while success permits borrowed inspection, validated path projection, or a
consuming transfer to the next owner. The legacy direct `write_output` route
is retired: compile options carry no product or publication policy, and ordinary
native publication consumes a retained product after compilation. The terminal
component driver still requires deployment inputs from their real owners. Its
tail accepts those independently acquired values as a
`TerminalComponentDeploymentSupply`, binds them to the staged
candidate, invokes the transaction, and constructs this report lane without
discarding deployment-stage or report metadata on failure. The supply does not
grant the compiler installation/provider/progress/profile authority. The driver
now invokes a `TerminalComponentDeploymentInputOwner` against the exact staged
candidate; acquisition rejection preserves that owner, the candidate, and all
report metadata, while success enters the typed transaction immediately. A
strictly compositional terminal driver now connects ordinary candidate staging
to that call: its staging carrier projects the checked-owned target/subsystem
choice and borrows the admission profile and exact provider settlements;
failure returns that carrier, the deployment owner, and report metadata, while
success proceeds through acquisition, deployment, publication, and report
custody. A separate typed terminal compile handoff now runs the ordinary
Psi-owned checked frontend and routes its result into this connected driver
without entering another backend coordinator. The checked result retains its
exact consumed source count and build-selected image subsystem. The request
retains compile options, optional package inputs, externally borrowed admission
and settlement evidence, and the external deployment-input owner: frontend
failure returns that request unchanged. The request itself now owns the
transactional checked-to-staging-input settlement: targetless binding returns
the original complete request beside the exact checked result, while success
produces one bound owner retaining checked/staging evidence and all external
custody. The driver no longer decomposes and reconstructs a loose request-parts
tuple, and later failure preserves established driver custody with options and
package inputs. The cutover adapter now binds staging target
and report metadata only from the owning `CheckedCompilation`: targetless
binding returns subsystem, admission profile, and provider settlements for
retry, while successful driving projects build evaluation and observation
metadata from that same owner. The connected driver invokes the same Psi-owned
artifact producer as the typed compiler product, then passes only that artifact
and explicit Omega realization facts into `stage_terminal_component`. A
concrete non-test installation/deployment
owner and an ordinary production caller supplying it remain prerequisites to
replacing the legacy publication path; the compiler has no authority to stand
in for that missing provider. This is platform/provider engineering rather than
a language-design block.
Compact record identities remain report keys and grant no authority.

### Placed-occurrence and resident-custody slice

Terminal Psi never serializes a concrete address as authority and never
re-resolves a source field, `P`, or `T`. Establishment produces an address-free
`PlacedOccurrenceId` bound to the exact qualified root occurrence, normalized
placement plan, provider/profile receipt, mapping and revision, range,
lifetime, and boundary reach. A separate `ResidentClaimId` names owned dormant
content. Each active owned view transfers that claim into one temporary placed
occurrence and resident-preserving retirement transfers the same claim back;
each borrowed view instead records an ordinary loan edge naming the exact
parent claim, range, polarity, and lifetime. Installation metadata records the
binding demanded by an artifact but is never itself authority: the installer
must consume the corresponding provider receipt and custody.

The concrete foundation now has separate provider-backed Stable and
Atomic-only owned resident lifecycles. The Atomic route consumes an exact
owned placement admission plus the non-Clone existing-content grant, replays
their plan/profile/resource, observation, origin, lineage, geometry,
provenance, era, claim, and receipt binding at every transition, and retains
the claim and one caller-supplied occurrence through projection and primitive
specialization. Resident-preserving retirement returns the same dormant grant;
rejection returns every input or carrier unchanged. This is still below
Terminal. Provider-backed Atomic resident content also supports shared and
exclusive whole-range borrowed views. Each active carrier retains the lender's
exact placement/profile/resource/admission and provider-content authority, the
unchanged resident claim and receipts, one caller occurrence, and the exact
loan polarity. Exclusivity adds no Atomic permission. Retirement replays that
authority transactionally, returns the complete active carrier on drift, and
releases only the loan without reminting custody. The borrowed carrier grants
no attempt, result, provider operation, lowering, or Terminal authority. A
separate non-authorizing carrier now joins an already-specialized
provider-backed observing request to its exact checked Atomic resident/result
contract. It independently replays the checked placed view and field plus the
runtime request authority; compares the complete retained placement structure,
not only its compact plan identity; requires the exact canonical field key,
width, claim, occurrence, decisive/single-attempt operation, and closed result
shape; and returns the unchanged non-Clone request on every rejection. The join
does not supply a source call or runtime result, attempt an operation, select or
install a provider, choose a comparison key or encoding law, create Terminal
authority, or lower to a target.

Each projected event retains the placed occurrence, normalized `P` and `T`,
canonical field key, already-normalized displacement and width, logical field
extent, physical effect footprint, operation family, plan receipt, and all
applicable lifetime, revision, reach, and custody identities. Backend lowering
receives only the bound base plus this retained displacement; it may not replay
layout evaluation or resolve names. The closed access families are Stable
read/take/write/swap, External read/take/write, atomic load/store/swap, the
observing and non-observing decisive and single-attempt compare-exchange
families, and the individual fetch families.

Pre-event specialization or installation rejection returns every input
unchanged and emits no access event. An admitted write event commits; a
physical access fault is a no-successor crash edge, never a program-visible
write rejection. Stable take transfers the exact resident field and leaves a
structurally partial occurrence. Stable swap returns displaced custody.
External take advances the external content version and returns one
provider-provenanced whole snapshot, introducing a content root only when the
snapshot's contract is content-bearing. Generic External swap is absent; an
authored provider exchange is a separate operation.

Custody classification is per exact claim row, not per operation. One event may
therefore contribute several differently classified rows:

- **introduction** has no parent claim and cites an exact authorized installed
  or provider-issuance occurrence plus its receipt;
- **identity forwarding** consumes one exact parent and produces that same
  claim occurrence;
- **derived transformation** consumes one or more parent claims and cites the
  checked theorem relating their exact content to the result;
- **custody exit or consumption** names the exact authorized sink for an input
  with no checked output; and
- **loan** is a non-owning edge carrying the exact parent claim, range,
  polarity, and lifetime.

These rows are independent inside one operation. A DMA completion may forward
the buffer extent, introduce provider-originated content into that buffer,
derive its new resident relation, and consume a completion token. The verifier
accepts no operation-wide label as a substitute for those claim-local facts.

The four flat generic atomic outcome declarations are live in core, and the
compiler-owned closed-shape inventory independently retains their exact two
axes, canonical case order, and payload disposition. This source-visible type
surface adds no Terminal operation or authority.
`AtomicCompareExchange<T>` is observing and decisive;
`AtomicCompareExchangeOnce<T>` is observing and single-attempt. Their failure
cases expose `observed: T`, so the resident must be copyable. The checked typed
placed-field boundary now retains this as a non-authorizing contract for each
admitted observing axis: exact field and resident type, normalized unrestricted
multiplicity, transfer width, permission identity, and the distinct decisive or
single-attempt closed result shape are independently replayed. Those shapes are
the flat core identities `AtomicCompareExchangeOutcome<T>` with canonical cases
`Mismatched(observed: T), Exchanged`, and
`AtomicCompareExchangeOnceOutcome<T>` with canonical cases
`Mismatched(observed: T), Exchanged, Uncommitted(observed: T)`. Affine or linear
observing residents reject. A `try_exchange*`-only field retains no such row and
gains no observing or selected-encoding authority. This contract is not a
Terminal row. The checked/runtime join may retain its matching provider-backed
resident request beside this row, but still carries no source call or runtime
result custody, atomic attempt or retry, key/encoding law, provider operation,
backend identity, or lowering authority. The independent non-observing pair,
`AtomicTryExchange<T, Key>` and
`AtomicTryExchangeOnce<T, Key>`, returns the proposed value on mismatch or an
uncommitted attempt and can transfer affine or linear residents. The copyable
key and selected raw-transition law determine the comparison without
constructing another owned `T`. Their result identities are
`AtomicTryExchangeOutcome<T>` and `AtomicTryExchangeOnceOutcome<T>`; every case
contains exactly one owned `T`, proposed on failure and displaced on success.
The law cannot erase Type-side custody: success always returns the displaced
resident, and ordinary multiplicity decides whether it may be discarded.
`Key` remains a copyable comparison input and its encoding law remains checked
call evidence; neither is a runtime outcome parameter. `Once` always denotes
the weak/single-attempt axis, never the observation axis.

Generic `ResidentContentTransfer<P, T>` is one provider requirement schema, not
one ambient slot per monomorph. An artifact records each concrete application
it uses and exports symbolic applications from generic code. Final composition
substitutes reachable generic arguments, derives the closed application set,
and verifies that the one selected provider covers every demanded application;
installation then binds the exact concrete occurrences. A separately indexed
slot family is introduced only when distinct applications genuinely require
independent provider selection.

D35 retires the representation-layer final-composition calculus that retained
an indexed arity/string schema and provider-authored generic or exact-family
assertion. Matching that assertion to closed demand did not establish a
checked realization. Terminal emission instead consumes D29's compiler-derived
tagged concrete and symbolic demands only after final specialization has
rejoined and rechecked the selected role-specific realization. Native replay
continues to bind the exact selected provider closure and source-free plan on
their own terms; no indexed-coverage identity is retained while the D29 join is
absent. Missing semantic coverage remains fail-closed and grants no provider
admission, resident custody, transfer, or installation authority.
