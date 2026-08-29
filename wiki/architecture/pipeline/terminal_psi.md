# Terminal Psi Architecture

[Pipeline](pipeline.md)

Status: target architecture settled 2026-08-02. This document describes the
current representation boundary. The semantic and evidence contract is owned by
[`canonical_ir_fuel_and_resource_provisioning.md`](../../design_briefs/canonical_ir_fuel_and_resource_provisioning.md).

This is a pre-release format. Producers and consumers move together; stale
artifacts reject. Git history records superseded vocabularies and implementation
checkpoints rather than this page accumulating a compatibility chronology.

## Boundary

Psi operates on Omega-branded source files and owns every target-neutral stage
through one canonical terminal representation. Omega consumes terminal Psi; it
does not feed source-shaped data back into Psi.

```text
Psi
    source files
    -> tokens -> syntax -> resolved -> typed -> checked
    -> lowered expressions, predicates, places, blocks, and edges
    -> terminal Psi

Omega
    terminal Psi
    -> verified optimizer unit -> optimized abstract operations
    -> target operations
    -> bounded physical assignment -> machine code -> object -> executable image
```

Compilation stops at the requested in-memory product. Publishing an executable
is a subsequent product operation that stages and replays the retained image
bytes; installation is a separate authority-bearing operation again. Neither
publication nor installation is a compiler fork.

Optimization is not optional control flow. An empty selected set is its identity
transformation: every native compile still reconstructs and validates the
verified optimizer unit, runs the bounded pass manager, and projects the result
before target lowering. The bounded assignment stage after target operations is
the one acknowledged transitional defect in this chain. It cycles scratch homes
for the small supported roster. The durable replacement is one in-line
continuation—target legalization, selected virtual instructions,
liveness/allocation, physical assignment, then emission—not a second backend.
Selected builds no longer reject before optimization: they consume the same
verified optimizer result and enter the in-line legalization,
selected-virtual-instruction, liveness, allocation, home, and
post-allocation-machine continuation. They fail closed only where that
continuation still lacks ordinary frame/exit, object/image, and publication
validation. The selected continuation must replace bounded assignment at that
join; compilation never falls back or runs an alternate route.

The ordinary realization graph uses the data-only `omega-program-entry-plan`
for source/target ProgramEntry declarations and
`omega-terminal-psi-to-native-artifact` for the shared composition edge. No
historical storage-wrapper or backend-coordinator route remains.

The Psi reference-interpreter entry and Omega abstract-operation entry accept
canonical semantic and proof sections plus an explicit admission profile,
decode and verify them, and only then construct resumable execution state or
realization requirements. No public in-memory module or checked-tree bypass
exists at either artifact boundary.

Parsing therefore belongs to Psi. “Omega files” is the language and product
branding; Psi is the frontend, semantic verifier input, and portable execution
representation.

### Boundary-argument realization fence

Ordinary in-module and bodyless boundary calls both carry positional scalar
arguments. In current terminal-Psi vocabulary 29,
`BoundaryMachineDeclaration` declares ordered scalar parameter types and
`BoundaryCall` carries the matching ordered `ValueId` arguments alongside its
structural lane. Canonical encoding binds
both orders; validation checks exact arity, definition, dominance, and type;
interpretation evaluates the scalar values before invoking the effect handler;
and Omega abstract lowering preserves them without reinterpretation. The
optional primitive scalar remains the independent result lane.

Vocabulary 27 also admits a first-class immutable borrowed byte-sequence shape,
an exact raw-octet literal establishment, and that local literal as a structural
argument to a bodyless boundary. The codec, verifier, and interpreter preserve
all bytes, including non-UTF-8 payloads. Psi syntax, resolved, typed, and checked
trees own that exact payload, and checked-to-terminal lowering establishes its
borrowed place before passing the same place to the bodyless call. In-module and
nonliteral forwarding remain fenced.

Vocabulary 27 also closes the O0 provider-backed attachment specialization. The
machine retains `attachment: Some(Main)`, its relevant `console` field retains
the exact erased provider identity, and sorted `ProviderAttachment` roots bind
that field to precisely the bodyless boundaries called through it. Validation
requires exact root/call equality and rejects missing attachments, duplicate or
orphan roots, runtime `self`, and provider roots forwarded as arguments.

Preservation is not realization. Omega target lowering accepts the one exact
Linux `exit_process(i32)` shape through import-free `exit_group`, including the
consumed scalar and nonreturning trap tail. It also accepts the exact literal-only
Linux `write_line` settlement: x86-64 and AArch64 emit an import-free short-write
loop over the retained bytes plus one newline, and object/image/installation
validation replays the code, data intervals, and structural custody. Other
nonempty calls and nonliteral byte sequences remain rejected; metadata-only
paths may not discard either lane. Darwin and Windows remain fail-closed pending
validated import/relocation evidence. Producers likewise may not encode an
effect input as an ordinary machine return or introduce a private pre-terminal
IR to evade this fence.

## Checked-adapter provider installation

A static bodyless boundary call keeps its boundary-machine ID in terminal Psi;
it is not rewritten to a chosen implementation. For the currently admitted
zero-argument Unit slice, Psi serializes every exact checked satisfier as an
ordinary terminal machine plus a canonical conformance row. The row binds the
boundary requirement identity, nominal provider identity, canonical adapter
identity, artifact-local machine ID, Unit signature, and checked service
refinement. Structural parameters, domain requirements, stateful provider
values, and completion receipts remain outside this slice.

Provider selection is not terminal-Psi semantic identity. Omega consumes its
retained `SelectedProviderPlanFacts`, resolves each selected `CheckedAdapter`
by exact overload, provider type, and adapter identity against the verified
catalog, and asks Psi to admit only those terminal IDs for the exact artifact.
The Psi interpreter follows a cataloged boundary only through that explicit
private-field installation; absence fails closed instead of falling through to
an external effect handler.

Provider-backed O0 roots use a distinct later binding. Terminal Psi retains the
authored attachment, erased provider field, bodyless boundary declarations, and
exact field-to-boundary roots, but does not serialize a chosen native provider.
Target lowering consumes separately admitted `ProviderExecution` values and
requires each execution's canonical requirement identity to equal the exact
boundary declaration before projecting any numeric execution record. The Linux
`write_line` and `exit_process` realizations therefore cannot be swapped merely
because both are effectful Unit boundaries.

An installation-bound boundary requirement may publish
`reaches <= Bound`. Terminal Psi retains a symbolic row keyed by that exact
requirement identity, the normalized `+`-union bound, and every internal
call-graph dependency on the row. It never serializes Boolean effect formulas,
a caller-authored provider choice, or one shared row inferred from equal
service sets. The selected provider plan supplies the concrete operation row;
installation verifies it is a subset of the bound and substitutes it through
the complete root closure. Preselection manifests report the unresolved row
and bound, selected manifests add the exact provider and operation, and final
admission rejects any unresolved row. Such a row cannot cross an ordinary
callable package or component boundary. Terminal verification reconstructs the
entry's reachable fixed boundary rows, primitive service uses, and exact
installation-dependency identities from executable operations. That derived
closure must equal the retained root declaration: missing, padded, stale, or
unused rows reject. A direct service use is not erased merely because the same
service also occurs in an abstract row's upper bound.

Trait requirements and explicit top-level `boundary requirement` declarations
retain distinct canonical requirement kinds. A top-level requirement keeps its
package-qualified operation, static telescope, signature, contract, and
visibility; a bodyless boundary machine, bounded row, or later selection cannot
synthesize that identity. Its checked or external satisfier is resolved by the
same exact provider-plan machinery as a trait requirement.

## The cut

`psi-checked-trees-to-terminal` is the sole executable semantic handoff.
`omega-psi-to-abstract-operations` consumes the verified artifact; unsupported
vocabulary rejects at that boundary. The former checked-tree, StateGraph, and
control-flow backend route has been deleted.

That consumer exposes four explicit entrances rather than one lowering
monolith: canonical artifact admission/replay, verified optimizer-unit
construction, provider-installation custody, and verified-machine lowering.
Machine lowering descends separately through payloadless recognition,
ordinary scalar/Unit lowering, and the bounded structural-result family.

Build orchestration may separately retain source-declaration receipts needed to
prove author intent, ProgramEntry identity, provider selection, and target
closure. Those receipts rejoin the canonical artifact in
`omega-terminal-psi-to-native-artifact`; they are not an alternate executable
representation and cannot supply operation semantics missing from Terminal Psi.
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
- explicit cleanup, transfer, conservation, invalidation, suspension, and
  boundary actions on edges;
- closed semantic operation variants, including scoped CPU/device ordering
  events; and
- fingerprinted contracts, obligation schemas, authorized admission sites,
  trust attribution, and work identities.

The first quotient correspondence carrier is proof-only. `TerminalModule`
retains a strictly identity-ordered table for the narrow monomorphic, total,
direct faithful `define` certificate. The codec serializes the complete
source-free certificate and rederives its retained identity on decode;
representation validation independently reconstructs its theorem,
correspondence, eligibility, and direct-result shape. A nonempty table is still
rejected by execution validation, owns no machine or operation, and
does not authorize a representative call. The explicit producer attachment is
therefore a canonical-retention prerequisite, not executable quotient
lowering.

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

Backend planning now performs the next address-free join after abstract
boundary lowering: every ordered demand retains the exact registrar occurrence
handle and exact ordered native-argument handle that own its `NativePlace`
root. Replay binds the original placement/thunk/demand evidence to the source
host-call site, registrar target and overload, lowering and call coordinates,
authored native order, nominal parameter identity, and boundary-plan
application fingerprint. Nested field
destinations preserve the complete nominal layout and ordered slot path without
deriving a byte offset; distinct fields may share one exact parameter root.
This evidence still carries no target operation, bytes, object relocation,
runtime address, registration authority, or lease.

The target-closed backend recipe now extends that join to the exact outbound
parameter `ValuePlacement` and one authoritative private-layout demand for the
current single-slot `Field` form. Layout/slot/requirement/data-symbol identity,
offset, pointer extent, and alignment replay against the selected target and
containing data layout. The offset is retained evidence, not source-authored
identity. Multi-segment physical path composition rejects as an engineering
gap, and direct-parameter construction remains synthetic-only pending the
settled source/native-telescope implementation.
That physical-destination recipe itself has no selected/assigned operation,
object symbol, relocation, bytes, runtime address, registration authority, or
lease; the exact registrar-native-parameter-to-assigned-operand join follows
separately.

That exact assigned-operand carrier now exists for custom/unknown outbound
registrar host operations. Selection records the exact source-call handle,
call/operation ordinals, and ordered native-parameter-to-abstract-operand rows,
retaining semantic-formal identity only where one exists and excluding the
result pseudo-argument. Target lowering resolves exactly one
retained occurrence and boundary edge and preserves abstract/target operand
handles; backend planning joins them to the assigned instruction and operand
for the prior physical destination. Replay rejects coordinate collisions,
missing or duplicated rows, stale handles, and operand-shape drift. Generic
host operations remain outside the carrier, and it grants no object,
relocation, byte, runtime, registration, or lease authority.

Object planning now permits one further evidence-only join for an exact
one-slot `Field` whose assigned registrar operand is
`RuntimeStorageAddress`. The ordered request binds that operand's storage
region/base and target-closed field geometry to the canonical BSS storage
symbol and binds the demand's function identity to one exact private text
symbol. Full snapshots replay against the rebuilt object plan; `DataAddress`,
direct parameters, ambiguous or malformed symbols, and geometry drift reject.
For this one-slot production shape, backend orchestration now inserts an exact
`WriteFunctionAddressToRuntimeStorage` operation contiguously before the
registrar call and rederives its target/assigned identity after every operation
arena rebuild. Both target encoders retain separate symbolic function and BSS
bases, relocation planning emits the architecture-specific pair(s), and final
replay independently binds function identity, storage symbol, sites, kinds,
addends, origin, cardinality, and unchanged instruction bits. The operation's
scratch/state demand is included in the validated root boundary footprint.
This grants no runtime registration, invocation, callback lifetime/lease, or
publication authority; `DataAddress`, direct parameters, and multi-segment
paths remain fenced.

Retained native artifacts now carry one ordered non-Clone callback installation
manifest. Each entry preserves the complete private object-store request and
checked placement identity, a domain-separated callback entry identity, exact
Text interval and BSS snapshot, encoded address store, and target relocation
rows; retained-artifact validation independently replays the full snapshot.
Deployment projects those entries into the artifact entry catalog, then binds
the complete sealed entry to a domain-separated digest of the exact installed
occurrence, architecture, unrelocated/materialized bytes, and entry offset.
Root installation requires the same installed-occurrence digest, entry, and
requirement, and pending/live/error/
cleanup/quiescence custody never drops that attribution. The manifest exposes
no resolved address and grants no registrar invocation, source `Registration`,
capacity, lease, or publication authority. `DataAddress`, direct parameters,
and multi-segment physical paths remain fenced.

Provider-execution metadata crossing target operations, machine code, and the
native artifact is an authority-free report projection. Its compact selected
plan, execution, normalized-root, and boundary-contract coordinates are named
report identities/fingerprints and preserve the existing installation wire
order. The admitted provider object borrowed at the lowering entrance remains
the authority carrier. The retained artifact additionally keeps the strong
selected-provider-closure digest and exact requirement strings/catalogs, and
its replay rejects exact-requirement substitution even when all compact report
coordinates are unchanged.

The current canonical checked-to-Terminal function has no input field for the
compiler's validated callback-placement sidecar. The compiler product driver
therefore accepts those rows for check-only output but rejects Terminal and
native artifact production before calling the Terminal producer. It reports
the complete row count and canonical callback identities rather than clearing
or reconstructing the sidecar. Canonical artifact production remains fenced
until this handoff has an explicit custody carrier and consumer.

Deployment now owns a separate two-phase reclaimable callback custody path. It
installs an independently admitted root before the ordinary registrar call and
retains the installed root plus exact ledger in a pending non-Clone carrier.
The later provider result establishes live registration custody only when its
receipt binds that exact root and reports success. Provider unregister and root
quiescence then complete transactionally and return the original slot
authority; every rejection retains the registration/root, ledger access, and
receipts needed to retry, while a false registrar result supports explicit
pending-root removal. The carrier now retains the exact installed callback-entry
attribution derived from the emitted store/demand manifest through pending,
live, rejection, cleanup, and quiescence results. It still does not invoke the
registrar, mint the source-level `Registration`, or supply live-registration
capacity.

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
Terminal rung now carries closed owned/shared/mutable/write-only access on
structural parameters and call arguments, with canonical format 27 identity.
That rung now includes one exact unrestricted `WriteOnlyBorrow` field-path
subloan. The verifier replays its ordered path, structural type, and access and
treats it as a claim-free non-transferring subloan rather than an owned linear
projection; malformed path, target type/access, source access, qualification,
arity, or provider substitution rejects. Reusable local or re-entrant reborrow
authority does not follow. The verifier also rejects widening, target
disagreement, overlapping exclusive arguments, and Boolean structural
observation through write-only access.
Executable Terminal stores, runtime/provider realization, and native lowering
remain gated; physical pointer-layout equivalence is not permission equivalence.

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
semantic phase batches and maintains an ephemeral available, suspended-by-
child, or retired-while-suspended state. A suspended carrier that weakens emits
no premature disposition. When the available descendant finally ends, one
checked-only row retains its exact child and parent resources, flow handles,
ordered retired-parent path, final retained-parent or direct-root-lifetime
target, and one of reactivate, cascade-through-retired-parent, or combined
retire/discard. Same-phase parent retirement selects the combined outcome;
arena order is irrelevant. This remains a non-authorizing replay carrier, not
proof that authority returned, became usable, was cleaned up, or crossed into
Terminal. In particular, it neither separates retirement from discard nor
supplies a Terminal resource row.

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

Nominal proposition declarations retain their binder telescopes,
fact-only/witness-bearing classification, and any normalized carrierless
evidence interface in this fingerprinted vocabulary. Changing that interface
is a semantic proof-API revision even though the proposition keeps its nominal
symbol. Transparent proposition definitions expand before terminal production,
have no independent semantic identity, and retain their source names only in
debug maps.

Witness-bearing declarations use the contextual
`proposition P(...) evidence Interface;` form. The normalized evidence
interface enters terminal proposition identity.

Witness-bearing facts additionally retain an evidence-term identity and a
separate derivation-provenance identity. Proposition terms are copyable;
consumable authority is represented by an affine or linear Type carrier.
Named `requires` inputs refer to exact positional erased terms; named `ensures`
outputs contribute public selectors to the erased proof-output lane. The
ordinary result remains on its declared Type and canonical runtime call;
outcome guards control which selectors are available in each arm. Producer
conformances remain inside proof construction and do not enter proposition or
output-lane identity.
Each guarded guarantee row carries the exact nominal case of the declared
result sum. Named rows additionally carry the public selector and evidence-term
identity; unnamed rows carry the proposition and derivation but no selectable
term. The source braces have no terminal row or identity. Verification requires
every ordinary exit producing the case to discharge the row by exact assignment
when named or by proposition proof when unnamed, rejects any discharge on a
different case, and requires caller fact import and selected term binding to be
dominated by the matching case refinement. The retained validity descriptor is
the intersection of the result occurrence, normalized referenced occurrences,
and evidence-interface scopes; codec replay preserves that descriptor and
write-invalidation coordinates independently from the case identity.
The bounded caller-side executable carrier preserves fact-only omission. A
zero-input `CallStructural` over a direct unrestricted, unqualified,
claim-free payloadless producer imports each guarded row as a case-membership
implication after exact result-place substitution. It imports neither a raw
conclusion nor a case-membership fact; structural return rebases the complete
implication to the caller result. One optional selected-evidence
binding may now retain the exact guarded case, row position, obligation, public
selector, atomic proposition, callee term/interface, distinct caller-local term,
and result-root validity intersection. Format 33 / vocabulary 35 encode those
coordinates. Validation requires the exact named callee row and producer
provenance and rejects identity/interface/dependency drift, duplicate output,
or unconditional/projection reuse. Omission remains fact-only, and the selected
binding still imports no raw conclusion or case fact. The bounded call rejects
ordinary contract lanes, crash rows, custody transfers, and callee evidence
contract lanes. Terminal interpretation transports the exact payloadless case,
fixed fuel composes to four units, and the binding adds no operation or charge.
The matching checked/source carrier accepts one attached zero-input direct
caller and producer over the same exact attachment. The caller captures the
call once in an immutable local and every exhaustive payloadless case arm
returns that saved result unchanged. Checked planning replays the exact flow
coordinate, target/receiver, symbol-root association, case coverage, and
result-root-only validity; at most one named row may be selected, while omission
stays fact-only. Lowering emits the exact two-machine closure, retains sibling
guarded rows and producer provenance on the callee, and rejoins the selected row
to a distinct caller-local term without changing the four-unit runtime. Payload
substitution, later guarded-term use, erased proof-output linkage, wider
structural calls, validity invalidation, and tagged-sum target lowering remain
outside this bounded source rung.
The current producer serializes forwarded terms as dense module-local
identities over the exact proposition application and a structured canonical
carrierless interface; the verifier requires each witness application to carry
that interface and each term row to agree with it. A forwarded output
contributes only its source vocabulary identity. Canonical positional rows for
the selected terminal machine's named `requires` and `ensures` lanes reference
those exact IDs, and forwarding places the same ID at both endpoints. The
verifier requires known machine/term IDs, dense positions per lane kind, and no
orphan term rows. A fresh ensured term is accepted only when the proof bundle
contains one canonical provenance row keyed to that exact term. The row has its
own proof identity and retains the selected conformance, evidence trait, and
complete normalized realization rows without source handles. Missing, unused,
malformed, reordered, or interface-mismatched provenance rejects. The row
changes the proof fingerprint, not terminal semantic identity, runtime, or
fuel. Each ensured lane also retains its public output selector beside the
exact `EvidenceTermId`; required lanes have no output selector, and missing or
duplicate names reject. The ordinary runtime result remains on its separate
`Call` operation. The retained carrierless interface includes its complete
direct
and inherited requirement surface, including each declaring trait's normalized
argument pack. A proof-static projection carries the canonical evidence-term
ID plus the exact declaring-trait application and requirement-overload
identity. Forwarding is canonicalized before applications are serialized, so
input and output aliases project the same opaque identity while separate terms
remain distinct. The verifier requires the term and exact row to exist in the
retained interface; diagnostic display spelling is never an identity oracle.
A dense invocation table retains the canonical caller and ordinal, normalized
callee-machine identity, explicit erased input lanes, and the selected
proof-output lanes in callee order. Each input names its target position,
formal proposition, exact caller source term, and call-substituted proposition.
Each output independently retains its formal and substituted propositions.
A producer-backed selector binds a distinct caller-local term and retains its
callee producer term. A selector that directly forwards one input instead
names that input position and aliases the exact supplied witness; it does not
invent producer provenance. Omitted selectors mint no caller term but their
substituted propositions still enter the caller's fact catalog. Repeated calls
share formal lanes and producers while retaining invocation-specific
substitution. Source coordinates and caller-local display names erase. The
execution shape is explicit: an erased proof-only row has no
runtime operation or fuel; a Unit-runtime row links one canonical `CallUnit`;
a scalar-runtime row retains its scalar result type and links one canonical
`Call`. The verifier requires each linked operation to occur in the declared
caller, have the declared result shape, and call the linked callee; a missing,
spurious, unknown, wrong-kind, wrong-caller, or mismatched-callee link rejects.
For a generic callee, the target-machine identity composes the checked
specialization report fingerprint and its strong identity, including every
concrete type, `const`, static-machine, and closed conformance selection. The
proof row therefore cannot alias another application that happens to retain the
same post-specialization callable shape. The proof row adds no operation or
fuel beyond that ordinary call.

Omega task activation applies the same authority split after checking.
`TaskRuntime::{start,try_start}` retains its compact specialization value only
as a report coordinate; provider planning derives a domain-separated strong
commitment over the exact checked TaskRuntime requirement and operation, exact
package-qualified target/entry signature including parameter modes, and target
machine-contract commitment. The task runtime receipt binding carries both
values but derives invocation identity from the strong commitment alone, so
compact equality never authorizes a different specialization.

For the first attached static trait-requirement proof call, the public target
is the requirement's normalized callable identity rather than the concrete machine.
A separate private dispatch row retains the caller-owned closed-conformance
application, its domain-separated commitment, exact declaring trait/
requirement/realization row, and emitted
Unit callee. The selected output has no satisfier callee-term or forwarding
coordinate: its requirement proposition and public selector authorize one
fresh caller-local opaque term. Representation validation rejoins the public
identity to the canonical row, the row to the owner-scoped application, and
the private realization to the ordinary `CallUnit`; it rejects missing
dispatch, identity, commitment, or report-fingerprint drift, private
forwarding/provenance leakage, and reuse of an input or prior output term. Codec
format 33 / vocabulary 38 preserve this split and serialize the application
and dispatch commitments. Terminal validation recomputes the application
commitment from its complete source-free structure, selects dispatch by owner
plus that commitment, and then replays the exact row. The compact fingerprint
is named a report fingerprint and remains report/index data only. Erasing the
proof rows leaves runtime parameter/result shape, storage, operations, and fixed
fuel unchanged.

Outcome
guards expose selectors only in applicable arms.
A selected generic conformance is already closed before Terminal Psi: its
identity retains the declared package-scoped name, complete normalized
telescope including any resolved elided lifetimes, instantiated subject and
trait application, and complete normalized row map. The terminal verifier
replays that exact application and rejects an open telescope, missing argument,
shape mismatch, or redirected row. Runtime Type results retain their ordinary
multiplicity independently of the proof lane; conformance selection adds no
runtime value, operation, or fuel.

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

The claim-free partial-cleanup slice accepts one affine transparent record. A
finite nonempty set of pairwise prefix-disjoint, nonempty all-field paths may
move through source-ordered one-parameter ordinary Unit calls, provided at
least one residual subtree remains. The Unit return then names every maximal
live residual subtree by exact root, canonical field path, and subtree type in
recursive reverse declaration order and never discards a partially moved
ancestor whole. The verifier independently proves that the moved and residual
paths are disjoint and exhaust the root. Interpretation
charges the return edge before disposing the residual paths, so fuel exhaustion cannot
clean early. Omega carries every path and type through all five target
pipelines, object/image validation, and canonical installation records while
emitting no cleanup instruction or runtime bitmap. Claims, content, contracts,
nominal `drop`, and arrays/cases remain fenced.

A separate exact fixed-array carrier accepts an owned, non-self, unqualified,
claim-free affine `[T; 2]` with structural record element `T`. One projected
Unit call may retain the opposite element as a typed no-code residual, or two
calls may move the exact path set `{[0], [1]}` once each in authored order. The
two-call form has one block and an ordinary `ReturnUnit` with no affine discard;
the ownership frontier removes the array root only after the second dense path
moves. Shape validation independently rejects duplicate or missing paths,
other lengths and element shapes, claims/content, and cleanup added to the
ordinary return; checked production separately excludes contracts. Interpreter
replay charges five closure units: two caller operations, two callee returns,
and the caller return. The empty
residual set makes no cleanup-order choice. Target and machine lowering plus
object/image and installation replay rederive this exact two-call form at
function scope, retain its canonical stride/offset and empty return cleanup,
and never exempt a projected call unless the complete pair is present.

The residual-bearing successor accepts the same exact carrier restrictions for
`[T; 3]`. It requires exactly two distinct direct literal-index calls in
authored order and one `ReturnUnitPartialAffine` discard naming the sole
complement index and exact element type. Shape and frontier validation reject a
missing, duplicate, third, nested, out-of-bounds, mistyped, qualified,
claim-bearing, contract-bearing, or cleanup-drifted form. Interpretation still
charges five closure units. Target and machine lowering plus object/image and
installation replay independently retain the length-three layout, stride,
offsets, two-call custody, singleton residual, and operation/edge fuel. Since
there is only one residual, this carrier makes no cleanup-order choice.

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
custody map, second join, or third predecessor rejects. General cycles also
reject; the exact ranked unsigned-countdown representation is the sole
exception. Its acyclic skeleton establishes the header frontier, one complete
covered cycle computes the preservation candidate, and all live claims, owned
places, and partial-custody paths must match exactly before representation
admission. The reference interpreter has a distinct validation and verification
carrier for only the one-machine structural Unit countdown: its proof scheduler
removes the already validated covered backedge, reconstructs the taken
`0 < remaining` edge as the discrete unsigned `1 <= remaining` subtraction
premise, and requires the exact-subtract evidence before constructing resumable
  execution state. Ordinary verification continues to reject the ranked machine,
  so fixed-fuel and Omega/native consumers cannot acquire authority through the
  interpreter path; provider installation and extra mixed work remain fenced.
  Fixed-fuel and native lowering instead use separate opaque verifier carriers
  for this exact machine. Native admission combines the canonical Terminal
  proof, exact fixed-fuel certificate, converged structural frontier, and the
  reconstructed preheader/header/guard/subtract/backedge graph into one custody
  object. Abstract, target, and assigned-target representations retain that
  object unchanged. Target lowering replays the exact graph, affine-owned
  structural parameter, exit cleanup, and ABI placement; assignment accepts
  only the target-prescribed rank register. The ordinary selected-instruction
  path stays closed, while a disjoint unoptimized route emits the exact Linux
  x86-64 and AArch64 countdown bodies from assigned custody. The machine-code
  carrier retains the semantic custody, complete ABI/structural inputs, and
  canonical four-operation/five-edge logical-fuel attribution. Object replay
  and generic native-fuel instrumentation remain closed: the former has not
  independently replayed the ranked body, and the latter cannot yet rebase its
  internal branches around inserted charge sites. This authority is not a
  general cyclic-control exception and cannot be obtained by converting either
  the interpreter carrier or ordinary acyclic verification.
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
nonempty path to a relevant Boolean field of a record parameter retains every
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
replay the exact common-field, case, and payload-field identities. Nested or
projected mixed values, recursive cycles, address and erased payload equality,
written `equals` bodies, and runtime sum layout remain outside this bounded
terminal slice. When an acyclic
relevant record field reaches a payload-bearing sum, the same sum proposition
is retained below that field path, and independent verification preserves the
complete `Field -> Case -> Field` identity chain. Direct source-call rebasing
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

### Normal result slices

A terminal machine result is `Scalar`, with one stable result pseudo-value;
`Structural`, with an exact type, multiplicity, qualification set, and result
place; or `Unit`, with no runtime value. `ReturnUnit` is a normal exit, not a
distinguished Boolean or integer: it creates no `ValueId`, result structural
place, or return-equality axiom. Contracts on a unit machine may refer to its
parameters but cannot name an absent result. Scalar and unit calls remain
distinct complete operation slices rather than ignored-result conventions.

Canonical encoding, independent verification, interpretation, and fixed-fuel
derivation implement this distinction. A plain unit return charges exactly its
one terminal edge and resumes atomically after sponsor exhaustion. Checked-tree
production and Omega native lowering now cover the bounded Unit, structural
custody, effect, and cleanup slices described above; unsupported Unit shapes
remain fail-closed rather than falling back to checked or source trees.

The first structural-result artifact slice is root-only whole-parameter
passthrough. `ReturnStructural` names the live source place and exact ordered
claim set transferred to the declared result place. Verification requires a
matching linear signature and whole-root entry/content binding, rejects result
places on scalar or Unit machines, and reconstructs content-identity facts only
at the validated return edge. Interpretation preserves the opaque value,
qualifications, and claim identities, charging fuel before custody or cleanup
commits; canonical encoding and fixed-fuel derivation cover the same edge.
The exact checked-source slice accepts one attached, one-state passthrough of a
whole linear parameter with matching qualifications and one whole-root claim.
It may additionally carry a finite tail of unqualified, claim-free affine
structural parameters, whose places are discarded after result materialization
in canonical reverse parameter order. It
may also establish a finite consecutive prefix of immutable, unqualified
empty-record affine locals before the return. Terminal Psi declares each local
without a source handle, charges each explicit establishment operation, and
cleans them in reverse declaration order before the optional affine parameter.
Declaration ordinals must be dense and establishment order exact. Nominal
cleanup, nonempty/partial locals, authored contracts, projections, and wider
cleanup/control shapes fail closed.

The first internal structural-call slice composes two such checked machines. A
`CallStructural` operation owns a structural operation-result place with its
exact type, multiplicity, qualifications, and caller claim bindings. The call
separately records the structural argument transfer and an exact map from each
returned callee claim to its continuing caller claim; the returned claim is a
continuation of custody, not a new establishment event. Verification requires
one whole linear argument, one whole callee entry/result claim, an immediate
structural return, and exact signature and content-identity correspondence.
The operation result and its claims enter the caller frontier only after the
callee returns successfully. Sponsor exhaustion is resumable without replay;
a callee crash creates no result. Canonical format 27/vocabulary 29 and the
fixed-fuel call closure retain the same relation. This slice deliberately does
not admit projections, several claims, local staging, bodyless structural
results, or wider native aggregate ABI lowering.

Omega realizes that exact slice through its target calling policy when the value
has one direct eight-byte integer fragment. The source and result placements,
typed local establishment, Psi edge, claim set, exact affine cleanup, and fuel
attribution survive target assignment, machine emission, object/image
construction, and canonical installation. Direct register and stack parameter
homes are retained exactly. The locals are not ABI parameters;
claim identity and trivial cleanup are zero-runtime semantic metadata rather
than extra ABI words or cleanup instructions. Wider or indirect values,
projections, additional/non-immediate structural calls, and broader control
remain fenced before partial lowering.

Normal scalar returns carry one exact ordered affine cleanup-action stream; the
stream is empty when no cleanup is required. Actions distinguish whole-root
no-code disposal, typed residual disposal, and executable nominal cleanup.
Verification reconstructs the complete live frontier and reverse declaration
order, and independently validates every nominal target and obligation.
Interpretation charges the return edge and materializes the scalar result before
committing actions, then runs nominal bodies resumably, so sponsor exhaustion
cannot replay a completed action or partially commit the exit. Fixed fuel
composes every nominal invocation. Omega preserves the same action order and
call ownership through target assignment, all five machine emitters, object and
image custody, and canonical installation; no-code actions emit no target
instruction. Current source production covers the wider trivial-discard scalar
slice plus the finite mixed no-code/nominal, branch-free-input/local branch described
above, including direct-Boolean contextual cleanup across mixed roots.

The proof kernel, proposition representation, total primitive judgments,
certificate envelope, and admission taxonomy land before an operation depends
on them. Concrete proposition and operation vocabularies are then co-designed
in vertical slices; the proof language is not speculated in isolation.

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
`CallStructural` carrier described above. Wider or bodyless structural results
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
`TerminalArtifact` product stops at this carrier without entering StateGraph,
native emission, output, or installation. Unsupported Terminal vocabulary
rejects there and cannot select another backend as a fallback.

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
The source-free native carrier belongs to the neutral
`omega-native-artifact` crate. The component-specific wrapper remains
in `omega-component-candidate`; compiler and deployment depend on
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

`AtomicCompareExchange<T>` is observing and decisive;
`AtomicCompareExchangeOnce<T>` is observing and single-attempt. Their failure
cases expose `observed: T`, so the resident must be copyable. The checked typed
placed-field boundary now retains this as a non-authorizing contract for each
admitted observing axis: exact field and resident type, normalized unrestricted
multiplicity, transfer width, permission identity, and the distinct decisive or
single-attempt closed result shape are independently replayed. Affine or linear
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
constructing another owned `T`; success returns the displaced resident unless
that law proves it discardable. `Once` always denotes the weak/single-attempt
axis, never the observation axis.

Generic `ResidentContentTransfer<P, T>` is one provider requirement schema, not
one ambient slot per monomorph. An artifact records each concrete application
it uses and exports symbolic applications from generic code. Final composition
substitutes reachable generic arguments, derives the closed application set,
and verifies that the one selected provider covers every demanded application;
installation then binds the exact concrete occurrences. A separately indexed
slot family is introduced only when distinct applications genuinely require
independent provider selection.

The representation foundation now has a non-authorizing final-composition
calculus for this rule. It retains an exact package-qualified indexed schema
and arity plus a generic or exact-family coverage assertion inside one already
selected provider closure, substitutes artifact-qualified symbolic parameters,
canonically closes concrete demand, and checks exact-family containment. Its
closed identity retains the selected provider closure, exact plan, application
set, and complete coverage assertion; multiple applications still name one
slot. Terminal emission of verifier-derived concrete and symbolic demand and
coverage rows, their composition wiring, and installation issuance binding
remain open. Native realization does retain the exact nonzero selected-closure
identity beside its source-free provider-plan projection. Component-candidate
replay requires both identities to agree independently, so indexed coverage or
resolved-reach drift cannot be masked by unchanged plan rows. Until the
remaining joins land, this structural closure and identity replay grant no
provider admission, resident custody, transfer, or installation authority.

### Crash-control slice

Terminal Psi represents `Trap` and `Abort` as closed crash causes attached to
distinct no-successor terminators. A crash terminator is not an ordinary
terminal transition and does not encode abandonment by omitting a cleanup list.
It carries a canonical site-guard set, covering published route buckets, and
the statically known local frontier as an explicit lower bound. The guard set
contains the exact incoming conjunction plus sound canonical consequences used
as route witnesses; retaining a consequence never erases the exact path
identity. The exact dynamically abandoned frontier is not claimed to be
edge-enumerable.

Published crash buckets are fingerprinted semantic content. Each bucket has
one cause and a canonical disjunction of route predicates over the same lowered
values and structural places as executable Psi. Buckets normalize by cause. An
unconditional clause contains the canonical `true` predicate.

The verifier checks each crash site against the canonical guard facts carried
by that site:

```text
the published route is Truth, or site_guard contains
    a canonical predicate from that same-cause route
```

Call composition substitutes arguments and caller path facts into published
routes. Disproving every route removes the corresponding crash edge from the
caller's semantic frontier. Evidence derived from a callee body is usable
only when that body is within the same fingerprinted verification unit.
Otherwise the verifier consumes the imported published ceiling and its
certificate.

The frontier lower bound is diagnostic and audit evidence only. It states which
tracked obligations are definitely live at the site; it cannot prove that
unlisted state or external effects remain valid, and no verifier may use it to
license survivors. Fault-tolerant restart requires separate closed-custody,
resource-recovery, external-reset, and target-isolation evidence.

The reference interpreter does not return a crash as data. Reaching a crash
terminator yields a distinct interpreter outcome carrying its cause and
semantic site identity. Build-time evaluation rejects any invocation with a
surviving crash route; a concrete invocation that disproves all routes remains
admissible. Native lowering preserves every reachable no-successor crash leaf.
It may retain a physical check even when a caller has proved its semantic edge
unreachable, unless specialization makes erasure valid.

These normalized obligations are semantic and fingerprinted. Their proof
derivations remain replaceable proof-bundle material.

## Verification boundary

The artifact verifier, proof kernel, and proof producer have distinct jobs:

```text
producer            emits canonical terminal Psi plus candidate evidence
artifact verifier   derives what that exact module is required to prove
proof kernel        checks derivations of those required propositions
```

The artifact verifier canonical-decodes terminal Psi, validates its structure,
derives structural obligations from every operation and edge, and retains the
fingerprinted author contracts. It matches evidence only after reconstructing
the complete obligation set. The proof bundle is not an obligation manifest and
cannot choose what is sufficient. Missing obligations, extra evidence, changed
propositions, wrong module/obligation identities, and unauthorized admission all
reject.

Each reconstructed obligation also carries its exact semantic subject. The
artifact root is refinement from the formal target operational system to the
canonical source operational system under one verifier-reconstructed
observation profile. Supporting rows may inhabit an exact intended
mathematical model or state global consequence over an exact theory, but they
join the root only through explicit checked bridge rows. Subject/model/theory
identity, semantics versions, target capsule, observation profile, bridge
dependencies, and admissions are canonical identity rather than proof-bundle
annotations.

The producer never supplies the required observation profile. The verifier
derives it from canonical semantics, boundary/component contracts, and the
consumer-selected deployment policy. Exact equality is the first sound replay
gate. A later cross-profile reuse path must carry a checked canonical forgetting
projection; two profiles may be incomparable. A profile omitting all
observations therefore cannot trivialize a nonempty reconstructed obligation.

The formal-target-to-silicon claim is outside this reusable artifact record. It
is a deployment-scoped admission that composes into the final trust report.
Reports distinguish checked facts, artifact-scoped admissions, and deployment
admissions and never render their union as an unqualified `verified` result.

Every accepted fact is:

- re-decided by a specified total kernel judgment;
- discharged by a checked certificate, carried or reconstructed by a total
  certifying procedure; or
- admitted at a sealed site and accepted by the consuming profile.

Admission cannot replace a derivable obligation. Search that may time out or
return unknown must carry its certificate for portable verification. Primitive
trusted judgments are minimized and each joins the enumerable language
soundness audit.

The semantic module, proof bundle, installation record, and debug/source maps
remain separate. Proof improvements do not change semantic identity; provider
selection and attached evidence do change their own section and container
identities. One execution verifies and runs the compiler's current Psi
vocabulary.

`psi-terminal-verifier` is the current Rust implementation of the artifact-aware
judgment and `psi-proof-admission` applies product-local checking and admission
policy to its per-artifact proofs. The deployment-
authoritative endpoint is a **canonical semantic-ledger generator**: one total
low-rung definition that consumes the canonical terminal-Psi bytes themselves,
rejects malformed structure, and emits the exact ordered goals and premise
introductions for that artifact. Trusting an AST decoded by Rust would leave the
decoder in the closure, so the low route begins at bytes.

The generator has five bounded responsibilities:

1. canonical decoding and validation of types, identities, SSA, control flow,
   calls, contracts, and closed operation tags;
2. direct denotation of each selected terminal operation into mathematics;
3. one canonical goal for every proof-bearing operation, edge, return,
   conservation event, contract, and admission site;
4. local premise introductions with their origin, prerequisites, establishment
   point, value/place versions, validity scope, and invalidating events; and
5. an acyclic logical-justification order covering every artifact node exactly
   once.

It performs no verifier-derived algebraic summary, normalization, interval
reduction, or multi-node composition. For example, three retained SSA
definitions cross the boundary as three local equations; an affine summary of
the three is a conclusion that untrusted automation must derive. Authored
compound contracts, a primitive operation's direct denotation, and checked
capture-free positional substitution at a call remain legitimate semantic
content rather than reduction.

Local operation meaning is specified by a closed, typed, declarative schema
table. Each leaf-operation row states operand and result well-formedness,
canonical denotation and goals, facts established after discharge, crash
behavior, and local fuel/frontier effects. The schema language admits no opaque
callbacks. The generator rejects a closed operation variant with no exact row.
Adding a leaf operation that fits the existing ledger algebra therefore adds
one isolated row; adding new control, effect, validity, or frontier machinery is
visibly a ledger-algebra revision rather than an innocuous table edit.

The large Rust analysis becomes an untrusted certificate producer. It may
discover affine forms, interval hulls, quadratic extrema, division preimages,
or any other sufficient argument, but it proves the canonical goal instead of
replacing it with the reduced proposition. The low-rung kernel checks that
derivation. A direct execution of the low-rung generator or a kernel-checked
derivation of that same total definition establishes the authoritative ledger
for every deployed artifact. Optimized implementations may disagree only by
causing rejection; agreement with the low result grants them no authority.

The current Rust migration now reflects that split for every scalar leaf.
Goal-free leaves, structural/effect leaves, calls, and the twelve proof-bearing
scalar leaves have separate exact-unique tables. The proof-bearing table owns
direct denotation, exact operand/result shape, six canonical goal shapes, the
normal-successor equation, crash behavior, fuel, and frontier policy. Artifact
reconstruction consumes one typed observation rather than reconstructing those
twelve local equations in operation-specific branches. The policy join now
binds eleven rows—exact arithmetic, divide/remainder and shifts plus
wrapping/saturating divide/remainder—to the shared integer-policy catalog by
primitive and domain identity; exact cast is the sole table row outside that
catalog. A separate migration
dispatcher still chooses the legacy sufficient proposition for eight rows and
is explicitly hashed into each affected reduction dependency. The four
wrapping/saturating divide/remainder rows instead select their canonical
proposition directly from their exact operation tags. This does not certify the
remaining reducers, and the current closure remains `fully-derived false`.

`NonzeroDivisor`, `ExactDivisionDefined`, and `ExactShiftCount` currently have
exact kernel-proposition projections. Unsigned fixed integers use `1 <= d` for both.
Signed nonzero uses the ordered disjunction `(d <= -1) OR (1 <= d)`. Signed
exact division/remainder uses the ordered disjunction `(d <= -2) OR (1 <= d)
OR ((d <= -1) AND (MIN + 1 <= n))`, where `n` is the dividend. Signed one-bit
nonzero uses only `d <= -1`; its exact-definedness goal is `(d <= -1) AND (0 <=
n)`, because neither `-2` nor `+1` inhabits that carrier. Exact shift counts
project the settled `[0, width)` law: known literals become `Truth` or
`Falsehood`, symbolic signed counts retain the ordered lower and upper bounds,
symbolic unsigned counts omit the carrier-implied lower bound, and a narrow
count carrier may imply the whole goal. Address carriers and mismatched operand
types reject. The other three canonical goal shapes remain deliberately
unprojected.

Exact right shift is the first shift-count production pilot. Reconstruction
selects the unchanged canonical proposition only when prior machine
requirements and pre-site semantic facts close every retained bound; the
generic untrusted producer builds exact citations and ordered conjunction
introduction, then the kernel checks the result. A missing bound or redirected
count identity does not gain canonical status and retains the versioned trusted
sufficient-reduction fallback. The operation's post-discharge result equation
is unavailable to its own proof. Exact left-shift representability is a
separate goal and remains unprojected.

For the four whole-row wrapping and saturating divide/remainder pilots,
reconstruction selects the nonzero goal solely from the exact operation tag.
An untrusted producer searches only the owning machine requirements and
verifier-reconstructed facts preceding the site, deterministically prefers the
signed negative arm, and materializes exact citation, integer-`<=`
transitivity, or literal-equality substitution evidence.
It kernel-checks the candidate before emission. Missing projection or proof
rejects; there is no evidence-dependent fallback to the legacy reducer, and
the operation's own result equation is not available to justify itself.

The complete carrier-total landed-literal family shared by exact divide and
exact remainder now reconstructs canonical `ExactDivisionDefined` directly.
An unsigned nonzero divisor literal or signed divisor literal other than zero
and `-1` selects this path solely from a prior semantic equality, and the
untrusted recursive producer proves the selected order arm from that citation
and a closed integer relation. The complete signed `-1` exceptional family
also selects the canonical goal when the dividend is independently landed as
any literal above the carrier minimum. Its certificate recursively composes
the third disjunct, or the two-conjunct `i1` goal, from both literal equalities
and closed order. The same canonical family also accepts an independently
retained exact `MIN + 1 <= dividend` proposition (`0 <= dividend` for `i1`)
from a machine requirement or pre-site semantic axiom. The producer cites that
exact proposition as the second recursive premise; it neither imports the
trusted reducer result nor infers a wider interval. Missing, stale, or weaker
bounds reject. A retained same-carrier literal lower bound `K <= dividend` is
also complete when closed order proves `MIN + 1 <= K`: the producer composes
that primitive judgment and exact prior citation through one checked
transitivity step. Reversed, mistyped, weaker, or wrong-dividend facts reject.
The direct safe-divisor family also selects canonical `ExactDivisionDefined`
from an exact prior `1 <= divisor` proposition for unsigned or signed fixed
carriers, or `divisor <= -2` for signed widths of at least two. Unsigned
certificates cite the goal directly; signed certificates cite and introduce
the selected first or second disjunct. The complete signed joint-bound family
selects the third canonical disjunct when both `divisor <= -1` and
`MIN + 1 <= dividend` are independently available through the supported exact
citation or checked transitivity paths. The producer proves both conjuncts,
constructs their conjunction, and introduces that ordered disjunct; either
missing premise or redirected operand identity rejects. A retained
`divisor <= -1` may also pair with an independently landed nonminimum dividend
literal; closed order and exact equality substitution prove its dividend-floor
premise. A minimum or wrong-identity landing rejects. Exact literal equalities
retained as machine requirements use the same complete substitution path and
are cited as assumptions rather than semantic axioms. The selector checks every
same-carrier equality; zero-only, minimum-dividend, mistyped, or redirected
premises reject. The complete endpoint-transport family pairs an exact retained
bound on `K` with an independently retained equality connecting `K` to the
canonical divisor or dividend endpoint in either orientation. The producer
cites both through integer-order substitution and changes only that endpoint.
Dividend transport stays inside the joint arm and requires its independent
`divisor <= -1` proof. A missing companion bound, unrelated equality, weak bound,
or changed untouched endpoint rejects. Signed `i1` may independently transport
both canonical conjuncts: `Kd <= -1` through `Kd == divisor`, and `0 <= Kn`
through `Kn == dividend`. Both substitutions remain mandatory; missing or
crossed equalities reject. A complete nested endpoint family may first derive
the canonical bound on `K` from one stronger retained bound and closed
same-carrier order, then transport it through equality. The producer nests
integer-order transitivity beneath substitution; weak bounds, missing
equalities, or wrong endpoints reject. The next complete nested family replaces
that closed side with a second exact citation: unsigned `1 <= M` and `M <= K`,
or signed `K <= M` and `M <= -2`, followed by `K == divisor`. The producer
nests two-citation transitivity beneath endpoint substitution in deterministic
ledger order. Missing or disconnected middle relations, weak signed ceilings,
redirected equalities, or wrong endpoints reject. The signed joint arm also
admits the complete dividend sibling: exact `divisor <= -1`,
`MIN + 1 <= M`, `M <= K`, and `K == dividend`. The producer constructs the
ordered conjunction, cites the divisor bound directly, and nests the two
dividend-floor citations beneath endpoint substitution. A missing or
disconnected middle fact rejects. The complete nested signed-`i1` family
transports both mandatory conjuncts from two exact citations each:
`Kd <= Md`, `Md <= -1`, `Kd == divisor`, and `0 <= Mn`, `Mn <= Kn`,
`Kn == dividend`. The producer emits the ordered conjunction of two
transitivity-under-substitution proofs; either missing middle relation rejects
the whole goal. The signed width-at-least-two joint arm is also complete when
both conjuncts use direct two-citation chains: `divisor <= K`, `K <= -1`, and
`MIN + 1 <= M`, `M <= dividend`. The producer introduces only arm 2 and
constructs its ordered conjunction from the two transitivity proofs; a missing
or disconnected citation rejects the entire arm. A signed `i1` divisor
fact alone remains
insufficient because the canonical conjunction also needs its dividend premise.

Fixed endpoint substitution is dispatched behind matching side-local
producer and reconstruction modules. Their `substitution/one` owners
independently enumerate the one-equality arm before the existing
`substitution/two` siblings. Equality orientation, source citation order,
endpoint identity, inner-relation precedence, outer proof shape, rejection,
and both fixed frontiers are unchanged; reconstruction does not consume the
producer's citation or proof node as authority.

The one-equality endpoint sibling now separates equality enumeration from
per-orientation completion. Independent producer and verifier
`integer_selection/substitution/one/completion` modules choose the matching
goal endpoint, rebuild the replacement relation through their own bounded
relation authority, and construct or replay the outer substitution. Their
`substitution/one` parents retain source citation and equality-orientation
order. Inner-relation precedence, substitution proof bytes, rejection, and the
fixed one-equality frontier are unchanged.

The fixed two-equality endpoint sibling now keeps its established
`integer_selection/substitution/two` API as a facade over independent
side-local `two/selection` owners. Each selection retains outer equality,
orientation, inner equality, then affine-relation order and exact fact
non-reuse. The producer constructs the same inner-then-outer substitution
bytes; reconstruction independently checks the final-alias affine relation.
Endpoint identity, rejection order, and the exact two-equality frontier are
unchanged.

The two-equality selection owner now also separates candidate eligibility from
per-candidate proof completion. Independent producer and verifier
`integer_selection/substitution/two/selection/completion` modules rebuild the
final-alias affine relation and construct or replay the inner-then-outer
endpoint substitutions. Their `two/selection` parents retain outer equality,
orientation, inner equality, exact fact non-reuse, and completion order.
Affine precedence, proof bytes, rejection order, and the fixed two-equality
frontier are unchanged.

Fixed two-equality endpoint-alias eligibility now lives in independent
producer and verifier `integer_selection/substitution/two/selection/aliases`
modules. Each resolves the exact goal endpoint, requires distinct same-carrier
Value roots, middle aliases, and target aliases, and accepts either inner
equality orientation. The `selection` parents retain assumptions-before-
semantic-axioms fact enumeration, outer orientation order, exact fact
non-reuse, and completion. Proof bytes, rejection order, and the fixed two-
equality frontier are unchanged.

The fixed one-alias order transport likewise keeps its established
`alias_transport/one` API as a facade over independent side-local
`one/candidates` owners. Each side retains assumptions before semantic axioms,
equality orientation, and indexed relation order before delegating endpoint
substitution completion. The producer alone materializes citation proofs;
reconstruction independently rebuilds the transported proposition. Proof
bytes, rejection order, and the exact one-alias frontier are unchanged.

The complete retained-bound `i1` family selects that conjunction when exact
prior `divisor <= -1` and `0 <= dividend` propositions are both present; the
untrusted producer cites both through conjunction introduction. A missing
premise or wrong operand identity rejects. Missing, reversed, weakened,
mistyped, or wrong-divisor facts reject. Missing
or excluded evidence rejects these paths. A complete two-citation transitive
family also accepts exact prior `1 <= K` and `K <= divisor`, or signed
`divisor <= K` and `K <= -2`, only with the exact shared middle term and operand
identity. The producer places both citations under one checked transitivity
node; missing, disconnected, reversed, or redirected pairs reject. An exact
prior canonical goal is cited directly; an exact prior canonical arm is
introduced only at its ordered disjunct index. Reconstruction uses the same
recursive `LessOrEqual`/conjunction/disjunction shape as the producer instead of
separate safe-divisor and exceptional selectors. Redirected goals, reordered
joint conjunctions, or wrong operands reject. No operation-result equation is
available as proof authority. The existing proof
rules and proof-bundle v19 codec carry these certificates without a further
vocabulary change. All other exact divide/remainder families remain
on their explicitly trusted sufficient reducer, and both complete rows retain
their current trust status. Their canonical proposition is settled, and the
existing kernel rules can check direct bound, disjunction, conjunction,
transitivity, and substitution proofs. The untrusted producer has one
kernel-checked recursive compositor for exact prior citations, atomic integer
bounds, conjunctions, and arbitrary
ordered disjunctions; this covers the common certificate spine, including the
three-arm signed exact goal and the i1 joint goal. It deliberately performs no
affine or interval analysis. The proof-kernel boundary now also exposes one
producer-visible `IntegerAffineWitness` checker for signed fixed same-carrier
definition chains. A witness cites a nonempty, strictly increasing list of
prior semantic-axiom rows and names a root and target; the checked result owns
the derived coefficient and offset.
The checker independently validates each cited equality, requires an SSA-value
root and exact add/subtract/multiply-by-literal steps, and recomputes the
normalized `A * root + B` form with checked arithmetic. Stale or reordered
indices, malformed definitions, carrier drift, unsupported roots, target
drift, ambiguity, and coefficient/offset overflow reject. This gives direct
definition chains and the affine branches used by same-root and correlated
analyses one common normalization-custody primitive. It is not a proof rule,
does not serialize into a proof bundle, and proves no integer bound by itself.
The same producer-visible boundary now checks the exact atomic target bound
obtained from a checked affine form and one canonical root `<=` proposition.
Positive coefficients preserve the root-bound orientation, negative
coefficients reverse it, and a zero coefficient deterministically retains the
cited orientation while mapping to the constant offset. The mapped endpoint is
recomputed with checked arithmetic and must inhabit the same carrier; wrong
root or literal shape, wrong target relation, arithmetic overflow, and
out-of-carrier endpoints reject. This conversion accepts no proof authority:
its caller must independently establish the supplied root-bound proposition.
`IntegerAffineBound` now composes those checks into one certificate node. Its
recursively checked child proves the exact root bound, while its
`IntegerAffineWitness` binds the root, target, strictly ordered semantic
definition indices, and one position-aligned optional literal-landing index per
definition. An absent landing means that the affine expression embeds the
typed signed literal. A present landing must be one strictly earlier exact
same-carrier equality between the selected non-chain SSA operand and that
literal. The kernel replays normalization, maps the child conclusion, and
records each landing before its definition in accepted premise closure.
Non-order or wrong-root children, stale/reordered/malformed words,
missing, late, redirected, ambiguous, or unused landings, target/carrier drift,
arithmetic failure, or changed mapped conclusions reject. Proof-bundle v19
retains tag 12 and canonically encodes the aligned optional indices; the
registered calculus is v16 and the Rust kernel v8, with the affine and cast
checkers included in both trust-graph source
sets.
The first bounded producer family uses the rule for one to five prior signed
fixed affine definitions whose exact retained root bound maps directly to a
canonical safe-divisor arm. Reconstruction and production enumerate shortest
words first and advance only prefixes accepted by the affine witness checker;
within each depth, semantic-axiom indices remain strictly ordered. The kernel
independently checks continuity, algebra, the mapped conclusion, and
accepted-premise custody. Missing root custody, incomplete, reversed,
redirected, or stale words, wrong targets, and noncanonical mapped arms reject.
Root custody may now also use one exact prior landed literal or value-alias
transport. A typed `root == literal` citation substitutes the root into either
endpoint of one closed reflexive relation; a value alias instead combines one
directly cited integer bound at the alias endpoint with its independently cited
equality. One exact two-citation order chain may instead reconstruct the root
bound through one shared SSA middle under a checked transitivity child. Direct
roots remain preferred, then landed literals, alias transport, and
transitivity; equality facts stay in ledger order, while bound and second-leg
indexes use their exact value endpoint. A missing bound, equality, or order
leg, unsafe or mistyped literal, identity, non-value, disconnected, redirected,
cross-carrier, or same-citation join rejects. Three-or-more-alias or
three-or-more-leg root reconstruction, words of six
or more definitions, joins, cast/shift compositions, and correlated results
remain trusted-reducer work; neither complete exact row changes trust.
An exact mapped affine bound may also close to the canonical arm through one
typed closed-literal order bridge on the unchanged target endpoint. A stronger
lower bound places the primitive bridge before `IntegerAffineBound`; a stronger
upper bound places it after. Candidate mapping supplies no authority: the
kernel rechecks the exact affine conclusion and the enclosing transitivity
certificate. A nonclosed, mistyped, redirected, or weaker bridge rejects, and
no variable-endpoint or cited-fact search is added.
Affine completion now lives in dedicated, side-local `affine_custody` modules.
Production and verification independently own the fixed five-definition
witness frontier, exact mapped bound, and optional closed relaxation; no
authority is shared.
Affine evidence selection now lives in dedicated, side-local
`affine_selection` modules. Production and verification independently preserve
the exact preference order across direct, literal-landed, fixed one-/two-alias,
and exactly-two-leg transitive custody before invoking affine completion; no
generic path search or additional evidence shape is introduced.
Prior-evidence primitives now live in dedicated, side-local
`integer_evidence` modules. Production alone owns citation indices and proof
nodes; verification independently resolves retained integer literals and
replays closed order. Selectors depend on these leaf helpers without sharing
authority, changing precedence, or expanding the search frontier.
Canonical integer coordination now lives in dedicated, side-local
`integer_selection` modules. Production independently builds the recursive
Truth/conjunction/disjunction/order proof shape before the public entry applies
the kernel check; verification independently replays canonical proposition
shape and fixed bound dispatch. Each preserves its prior precedence and finite
evidence frontier.
Certificate-entry custody now lives in dedicated, side-local
`certificate_entry` modules. Production exposes a selected proof only after the
kernel accepts its exact context, goal, assumptions, and semantic axioms;
verification independently projects the canonical scalar goal before retained
selection. Invalid projection or failed checking yields no authority, and
neither side imports the other's decision.
The producer's 30 certificate regressions and verification's 25 independent
selection regressions now live in side-local `tests` modules. Production
facades are 35 and 608 lines respectively, while every test name and assertion
is retained; no proof logic, authority, precedence, or search frontier moved
between sides.
Verification control-flow evidence propagation now lives in a side-local
`path_facts` module. It alone decodes retained condition predicates, binds
successor parameters, emits edge equalities before rewritten facts, and
deduplicates propagated facts. The reconstruction parent still owns traversal,
merge intersection, and certificate selection; this extraction grants no proof
authority and changes no fact order.
Per-operation obligation reconstruction now lives in a side-local
`operation_facts` module. It preserves the exact goal-free, proof-bearing,
structural-effect, then call dispatch order; only the proof-bearing branch may
choose canonical certificate custody or trusted sufficient reduction before
recording the pre-result axiom snapshot. CFG traversal and return intersection
remain in the parent, and an unclaimed validated operation still fails closed.
Terminator custody now lives in a side-local `terminator_facts` module. It owns
the exact Jump/Conditional/return/crash dispatch, successor fact propagation,
scalar-result equality, nominal-cleanup obligations, structural-return facts,
and the rule that Crash contributes no normal exit. CFG scheduling and final
all-return intersection are separately owned below; cleanup order, axiom
snapshots, and noncanonical cleanup status are unchanged.
Immutable machine reconstruction context now lives in a side-local
`machine_context` module. It alone derives the existing path-fact enablement
predicate, exact value-type proposition context, machine-parameter custody set,
and block/machine identity indexes. Traversal consumes that read-only context;
operation and terminator modules retain their independent decision authority,
and no dispatch, fact, proof, or search order changes.
Deterministic machine fact flow now lives in a side-local `machine_flow` module.
It owns the existing sorted-ready topological schedule, per-block all-incoming
fact intersection, and final all-return fact intersection. The parent retains
operation-before-terminator traversal; no successor, fact, exit, proof, or
search order changes.
One exact prior value equality may also transport a completed affine bound from
its checked target alias to the canonical goal endpoint. The producer replaces
that one endpoint, constructs the bounded affine relation directly, and wraps
it in `IntegerLessOrEqualSubstitution`; reconstruction repeats the same exact
identity selection. A missing, redirected, crossed, or mistyped target equality
rejects. The affine relation builder cannot recurse into another target alias,
so this adds one wrapper only and no alias-chain search.

One fixed sibling may instead carry a completed affine bound across exactly two
distinct same-carrier target equalities. It nests two
`IntegerLessOrEqualSubstitution` nodes outside `IntegerAffineBound`; missing,
reused, redirected, cyclic, or mistyped equalities reject. The constructor
builds the affine relation directly at the final alias and never recurses
through the general order prover, so a third target alias remains outside the
family.

One bounded mixed root-custody sibling may instead compose exactly two prior
order citations at an alias endpoint, transport that completed bound through
exactly one retained value equality to the affine root, and then apply
`IntegerAffineBound`. Its proof nests `IntegerLessOrEqualTransitivity` beneath
`IntegerLessOrEqualSubstitution`; missing or disconnected order legs and absent
or redirected equalities reject. The constructor calls the affine builder
directly, so it cannot add another equality or order leg and does not introduce
recursive path search. Three-or-more-alias and three-or-more-leg custody remain
outside the producer.

One fixed two-alias sibling may instead transport one directly cited bound to
the affine root through exactly two distinct retained value equalities. Its
proof nests two `IntegerLessOrEqualSubstitution` nodes beneath
`IntegerAffineBound`; the root, middle alias, and bound alias must be distinct
same-carrier values. A missing, reused, redirected, crossed, cyclic, or mistyped
equality rejects. The constructor has no recursive alias walk, and a third
alias remains outside the producer.

Generic fixed two-alias transport now places its ledger/index enumeration in
independent producer and verifier `alias_transport/two/candidates` modules.
The unchanged `alias_transport/two` entry APIs still accept the final
completion callback. Outer equality, orientation, inner equality, then indexed
bound order, exact fact non-reuse, nested substitution bytes, callback order,
rejection, and the two-alias frontier are unchanged; reconstruction builds its
own retained bound rather than consuming producer evidence.

One literal-ending sibling may land the affine root through exactly one
intermediate value alias and one exact same-carrier literal equality. It proves
a closed reflexive integer order, substitutes the alias, substitutes the root,
and only then applies `IntegerAffineBound`. Missing, redirected, reused, or
mistyped equalities reject, and a second value alias is not followed. This is
another fixed two-substitution path, not a recursive alias search.

The common pure-cast spine now has the same kind of producer-visible custody.
`IntegerCastChainWitness` binds one or more contiguous partial fixed-native
`IntegerExactCast` definitions to exact root and target SSA values. The checker
requires strictly increasing semantic-axiom indices, canonical result-equality
orientation, exact adjacent source/target continuity, and the same 8/16/32/64
fixed-carrier partial-edge rule used by the accepted cast sandwiches. It
retains the complete carrier word and computes the exact intersection of every
carrier as the surviving mathematical root interval. A narrowing or cross-sign
edge is therefore never claimed total or lossy: only values in that
intersection survive. Identity, widening-shaped, address, non-native, stale,
reordered, reversed, discontinuous, cyclic, and target-drifted claims reject.
The checked core covers both one-cast sandwiches and the contiguous multi-cast
spine shared by computed-prefix, computed-suffix, and two-sided families.

This cast checker accepts no proof authority, does not establish that its root
is a machine parameter, and does not validate the surrounding prefix/suffix
algebra. Heterogeneous words containing `IntegerWiden` require their own
normalization witness. No cast result, carrier interval, or selected axiom is a
certificate premise until an intentionally versioned proof integration binds it
explicitly. `IntegerCastBound` binds one recursively checked root-bound child
and one nonempty contiguous word of partial casts. The word maps the same
mathematical literal endpoint into the final carrier; the kernel rechecks the
complete cast witness and conversion and records every definition in accepted
premise closure. A non-order or wrong-root child, empty, stale, reordered,
discontinuous, total/widening-shaped, or cyclic cast definitions,
target/orientation drift, or a changed endpoint rejects. Proof-bundle v19
retains tag 13; the registered calculus is v16 and the Rust kernel v8. Producer
and reconstruction independently follow the unique exact-cast SSA definition
spine backward from the goal, reject ambiguous target definitions, and require
its source-ordered ledger word. They perform no recursive path or permutation
search. Cast-chain custody now lives in dedicated, side-local `cast_custody`
modules. Production and verification independently own unique-spine selection,
exact witness/kernel replay, and final `IntegerCastBound` completion; the
broader evidence selectors retain their existing order and proof shapes. Cast
evidence selection now lives in dedicated, side-local `cast_selection` modules.

Each side's cast-chain owner now separates its two deterministic spine duties
behind the unchanged `cast_custody/chain` API. Side-local `chain/definitions`
modules recover a word between an already-selected root and target, while
`chain/source` modules discover the unique non-cast source and first cast
position. Both retain backward ledger traversal, ambiguity/reuse rejection,
source-order validation, and the semantic-axiom-length cycle bound. Producer
and reconstruction still perform these walks independently; neither path adds
alternate-edge or permutation search.

Exact-cast custody completion now separates ordered goal-target enumeration
from per-target witness replay. Producer and reconstruction
`cast_custody/completion` parents retain left-endpoint before right-endpoint
order and value eligibility; independent side-local `completion/target`
modules recover the exact cast word and construct or check its bound
conversion. Only production materializes and kernel-checks `IntegerCastBound`.
Proof bytes, per-target rejection, and the finite unique-spine frontier are
unchanged.

Production and verification independently preserve direct-bound,
landed-literal, fixed one-alias, closed-strengthening, alias-landed-literal,
then fixed two-alias precedence; source-carrier literal remapping remains with
cast custody. No proof shape or search frontier changes. This completes
contiguous cast-chain custody for exact divide/remainder goals.

Direct retained-bound cast selection now separates source-ordered relation
enumeration from its existing completion. Independent producer and verifier
`cast_selection/direct/candidates` modules retain assumptions before semantic
axioms and exact `LessOrEqual` filtering before delegating the selected
relation to their own completion. Citation identity, root-endpoint order,
`IntegerCastBound` proof bytes, rejection, direct-before-literal precedence,
and the fixed direct-bound frontier are unchanged.

Direct landed-literal cast selection now separates equality candidate
enumeration from its existing completion. Independent producer and verifier
`cast_selection/literal/candidates` modules retain assumptions before semantic
axioms, equality orientation, and exact typed value/literal filtering before
delegating to their own completion. Citation identity, closed-order and
substitution proof bytes, unsafe or mistyped rejection, cast-family precedence,
and the fixed direct-literal frontier are unchanged.

Closed-strengthened alias transport now separates fact discovery from its
existing cast completion. Independent producer and verifier
`alias_transport/cast/stronger/candidates` modules retain equality-first,
orientation-second, then bound-order enumeration. Their parents still invoke
side-local completion with the same
cited proof nodes or retained facts. Citation identity, closed bridge and
substitution bytes, rejection order, single-alias/single-bridge frontier, and
cast-family precedence are unchanged.

Stronger alias-bound endpoint eligibility now lives in independent producer
and verifier `alias_transport/cast/stronger/candidates/bound` modules. Each
requires the selected alias at the left endpoint before the right fallback,
decodes the opposite endpoint as a fixed integer literal, and requires its
carrier to match the root. Candidate parents retain equality, orientation, and
bound citation order. Completion inputs, proof bytes, rejection, and the fixed
single-alias/single-bridge frontier are unchanged.

Alias-landed-literal transport uses the same ownership split. Independent
producer and verifier `alias_transport/cast/literal/candidates` modules retain
root-equality-first, orientation-second, then distinct landing-equality order,
including exact fact non-reuse and carrier eligibility. Existing completion
owners receive the same two cited proof nodes or retained terms. Equality
identity, nested substitution bytes, target-endpoint order, rejection, the
single-alias/single-landing frontier, and cast-family precedence are unchanged.

One separately
bounded source-affine composition may now supply the cast root-bound child:
production first follows the unique cast spine to its non-cast source, remaps
the canonical literal endpoint into that source carrier, and invokes the
existing finite affine selector only on semantic axioms strictly before the
first cast. It then nests the accepted `IntegerAffineBound` beneath
`IntegerCastBound`. Reconstruction independently repeats the unique-spine,
endpoint-remap, prefix-boundary, affine, and cast checks. A missing or ambiguous
cast source, an affine definition or landing at/after the first cast, an
unrepresentable endpoint, or either failed kernel check rejects. Direct,
literal, and fixed alias cast families retain precedence. No rule or proof
schema beyond proof-bundle v19 is added. This bounded composition does not
promote either whole row.

This affine-to-cast selector now separates entry dispatch, endpoint
orientation, and resolved completion. Independent producer and verifier
`cast_selection/affine` facades delegate to side-local `affine/candidates`
modules that retain right-endpoint-before-left-endpoint order and value
eligibility. Their `affine/completion` siblings own source-spine recovery,
literal remapping, prefix-bounded affine custody, and cast completion. The
source goal, proof bytes, rejection order within one orientation, direct-cast
precedence, and finite frontier are unchanged.

The bounded dual may start from one directly cited
same-carrier source bound, replay a unique nonempty partial-cast spine, and then
complete one later finite affine word. Production remaps the cited literal into
the cast target, constructs `IntegerCastBound` directly from that exact
assumption, and accepts `IntegerAffineBound` only when every affine definition
and optional literal landing lies strictly after the final cast. Reconstruction
independently repeats those steps and the same strict boundary. Existing affine
families retain precedence. Missing source custody, an ambiguous cast spine,
unrepresentable endpoints, or an affine definition/landing at or before the
final cast rejects. The proof is exactly
`IntegerAffineBound(IntegerCastBound(Assumption))`; no rule or v19 schema field
is added. Shift/cast, joins, correlated results, and all other affine/cast
shapes remain trusted-reducer work, and `fully-derived false` is unchanged.

One exact forward affine/cast/affine sibling may now compose both bounded sides.
It starts from one directly cited same-carrier root bound, maps that exact
endpoint through one finite affine word strictly before the first partial cast,
replays the unique nonempty cast spine, and maps the same endpoint through one
finite affine word strictly after the final cast. Production enumerates only
the established fixed affine witness frontier and constructs
`IntegerAffineBound(IntegerCastBound(IntegerAffineBound(Assumption)))`;
verification independently repeats both affine witnesses, both strict source
boundaries, the cast word, and every endpoint conversion. The forward mapped
propositions are candidates only: each existing proof rule rechecks its exact
child and conclusion. Missing direct root custody, ambiguous cast custody,
unrepresentable mapped endpoints, or any affine definition or literal landing
on the wrong side of the cast rejects. Existing one-sided families retain
precedence. No inverse arithmetic, alias walk, proof rule, or v19 field is
added; broader affine/cast shapes and the complete exact rows retain their
trusted status and `fully-derived false` remains unchanged.

Cast-adjacent affine selection is now split at that exact responsibility
boundary on both sides of the trust boundary. Small producer and verifier
parents retain direct-before-sandwich dispatch, while independent side-local
`cast/direct`, `cast/sandwich`, and `cast/endpoint` modules own direct root
completion, the fixed two-affine composition, and typed literal remapping.
Citation order, strict cast boundaries, proof shapes, rejection behavior, and
the finite frontier are unchanged; neither side shares evidence authority.

The direct cast-to-affine sibling now separates entry dispatch, cast/root-bound
enumeration, and resolved completion. Producer and verifier `cast/direct`
facades delegate to independent `cast/direct/candidates` modules that retain
semantic cast-root order, unique source-spine recovery, and requirement order.
Their `cast/direct/completion` siblings own endpoint remapping, exact-cast
completion, and the strictly post-cast affine suffix. Assumption identity,
proof bytes, last-cast boundary, rejection within each candidate,
direct-before-sandwich precedence, and the finite frontier are unchanged.

The fixed affine/cast/affine sibling now separates entry dispatch, candidate
enumeration, and proof completion. Producer and verifier `cast/sandwich`
facades delegate to independent side-local `cast/sandwich/candidates` modules
that retain semantic cast-root order, exact source-spine recovery, and
requirement/root-endpoint order. Their `cast/sandwich/completion` siblings own
the mapped-prefix, exact-cast, then affine-suffix composition. Citation
identity, strict first/last-cast boundaries, the nested proof shape, rejection,
and the fixed frontier remain unchanged.

Boundary-aware affine custody is likewise split behind unchanged parent APIs.
Independent producer and verifier `affine_custody/boundary` modules own strict
post-boundary completion, while side-local `affine_custody/mapped` modules own
exact pre-boundary mapping to a requested target. The parents retain ordinary
root completion. Definition and literal citation order, strict inequalities,
mapped propositions, proof shapes, rejection behavior, and the fixed
five-definition frontier are unchanged; reconstruction still derives and
checks its mapped proposition independently.

Pre-boundary affine mapping now also separates target candidate enumeration
from per-witness completion. Independent producer and verifier
`affine_custody/mapped/completion` modules retain the strict definition and
literal-axiom boundary checks, validate the witness, and construct or replay
the exact mapped bound. Their `mapped` parents keep the requested-target and
definition-word order. Proof bytes, rejection within each candidate, and the
fixed five-definition frontier are unchanged.

Post-boundary affine custody now mirrors that responsibility split.
Independent producer and verifier `affine_custody/boundary/completion` modules
retain the strict definition and literal-axiom boundary checks and delegate an
eligible witness to their own ordinary affine-custody completion. Their
`boundary` parents keep goal-target and definition-word order. Proof bytes,
rejection within each candidate, and the fixed five-definition frontier are
unchanged.

Affine-witness candidate coordination now separates goal-target enumeration
from exact fixed-target completion. Independent producer and verifier
`affine_custody/candidates/fixed` modules align literal landings and construct
their own witness candidates for one requested target. Each parent computes
the bounded definition-word frontier once, then retains target-first and
definition-word-second order. Literal alignment, completion callbacks,
rejection, and the fixed frontier are unchanged; neither side shares a witness
or enumeration authority.

Unique earlier literal-landing discovery now lives in independent producer and
verifier `affine_custody/frontier/prefix/literals/landing` modules. Each scans
only the semantic-axiom prefix before its affine definition, validates every
row, preserves row order and both equality orientations, and accepts exactly
one same-carrier Value-to-signed-literal match. The `literals` parents retain
definition-word replay, arithmetic-step orientation, sibling position, and
target completion. Witness bytes, missing/late/redirected/ambiguous rejection,
and the fixed five-definition frontier are unchanged.

One landed affine-sibling definition step is now decoded by independent
producer and verifier `affine_custody/frontier/prefix/literals/step` modules.
Each requires an exact same-carrier Value target, accepts only exact integer
add/subtract/multiply, preserves left-operand precedence, and permits the right
operand only for commutative add/multiply. The `literals` parents retain word
traversal, unique landing alignment, and final-target completion. Witness
bytes, arithmetic orientation, rejection, and the fixed frontier are unchanged.

The cast root-bound child may also be reconstructed from exactly one retained
same-carrier `root == literal` fact when that literal equals or strengthens the
canonical bound endpoint. Production remaps the endpoint into the source
carrier, checks the closed bridge to the landed literal, substitutes the root
endpoint once, and then applies `IntegerCastBound`; verification independently
selects the same exact equality and rechecks the bridge. Direct bounds remain
preferred. Missing, redirected, mistyped, or weaker facts reject. One exact
same-carrier `root == alias` citation may instead transport one directly cited
canonical bound at that alias. Its fixed proof nests one
`IntegerLessOrEqualSubstitution` under `IntegerCastBound`; verification repeats
the same exact equality/bound selection. Missing, redirected, cross-carrier, or
weaker bounds reject. Production routes this one-alias order transport for both
cast and affine completion through one indexed constructor; verification
independently mirrors that constructor, so the family is no longer
re-enumerated per completion rule. One closed source-carrier endpoint bridge
may also strengthen the cited alias bound. Its fixed proof nests
`IntegerLessOrEqualTransitivity` under the one substitution; exact alias bounds
remain preferred. Production and verification recheck the same bound, bridge,
and equality. They do not search alternate bounds or aliases, and a weaker
bridge rejects. One fixed sibling may instead land that alias through exactly
one same-carrier `alias == literal` citation. It proves the closed canonical
bridge, substitutes the alias, substitutes the root, then applies
`IntegerCastBound`; production and verification select the same two exact
equalities. Missing,
reused, redirected, mistyped, or weaker literals reject. A second alias,
affine/cast, shift/cast, joins, and correlated results remain outside this
sibling. One separate fixed two-alias sibling may transport one directly cited
canonical bound through exactly two distinct same-carrier value equalities. It
nests two `IntegerLessOrEqualSubstitution` nodes under `IntegerCastBound`;
production and verification independently enumerate that exact three-citation
shape through their own local indexed constructor shared by cast and affine
completion. Those fixed one-/two-alias constructors now live in dedicated,
side-local `alias_transport` modules rather than the broader certificate and
reconstruction engines. The cast-specific closed strengthening and
alias-landed-literal shapes live beside them while retaining their distinct
transitivity and substitution proofs. They prefer every one-alias family and
perform no recursive or parameterized alias walk.
Missing, reused, redirected, crossed, cyclic, mistyped, or weaker facts reject.
A third alias and literal landing through two aliases remain outside. Neither
complete exact row changes trust and `fully-derived false` remains.

The fixed one- and two-alias affine branches now share one side-local custody
handoff after their distinct bounded selectors finish. Independent producer
and verifier `affine_selection/alias/completion` modules accept the selected
root and transported bound and invoke their own affine custody. The
`affine_selection/alias` parents retain one-alias before two-alias dispatch;
alias enumeration, citation identity, substitution proof bytes, rejection, and
both fixed frontiers are unchanged.

The common exact-shift spine now also has a producer-visible, non-serialized
`IntegerShiftChainWitness`. It binds a nonempty ordered word of exact left and
right shifts over one fixed-native SSA value carrier. Every step names its
canonical operation equality and, for a nonclosed count, an earlier canonical
equality landing that exact count. The checked form retains shift direction,
heterogeneous fixed-native count carrier, mathematical count, operation index,
and optional count index. Operation indices must strictly increase; each count
must be nonnegative and less than the value width, and each cited count fact
must precede its operation. This one ordered representation covers homogeneous
left/right and mixed shift cores shared by direct, cast-adjacent,
affine-adjacent, and divide/remainder-adjacent families. It deliberately has no
cumulative-count summary because mixed left/right composition is order
sensitive. Unsupported carriers, nonexact operations, unlanded, late,
reversed, mistyped, negative, or out-of-range counts, stale or reordered
definitions, discontinuity, cycles, and target drift reject.

The shift checker accepts no proof authority, does not establish machine-root
custody, and proves neither left-shift overflow safety nor any surrounding
preimage or interval claim. Cast witnesses remain non-certificate custody;
affine witnesses become certificate premises only inside a checked
`IntegerAffineBound` node.

The legacy trusted exact-left mixed-chain reducer now selects its complete
latest-definition walk and exact earlier landing index for every nonclosed
count, reverses that word into root-to-target order, and invokes the common
checker. It computes the unchanged interval preimage only from the checked
value carrier, root, direction, and mathematical count rows. The reducer no
longer independently interprets each selected definition's direction/count.
This is checker consumption rather than proof promotion: the reducer remains
trusted, its left-shift safety goal and certificate routing are unchanged, and
no other direct, cast-adjacent, affine-adjacent, or divide/remainder shift family
switches custody.

The correlated affine exact-divide/remainder family now has one complete
non-serialized `IntegerCorrelatedForbiddenRootWitness`. It independently
replays both nonempty exact add/subtract/multiply branches backward from the
dividend and divisor, including exact prior canonical equalities for nonclosed
landed siblings. The definition walks must be disjoint, source ordered, and end
at the same direct signed fixed-native signature parameter with nonzero checked
coefficients. The checker deterministically reselects the tightest strict unary
lower and upper signature bounds after the definition boundary, binds their
exact axiom identities, and solves the divisor's integer-lattice zero and `-1`
roots. The `-1` root is forbidden only when the checked dividend form evaluates
to the carrier minimum there. No forbidden root reconstructs the exact
two-bound conjunction; roots covering the whole retained interval reconstruct
falsehood; partial safety rejects. Stale definitions or landed siblings,
branch/correlation/order/type/root drift, one-sided or redirected bounds,
constant branches, and checked arithmetic failure reject.

The legacy trusted exact divide/remainder reducer now selects the complete
branch-definition and landed-literal coordinates plus the deterministic tight
lower/upper axiom indexes, constructs this witness, calls the independent
checker, and returns only its reconstructed sufficient conclusion. The prior
duplicate divisor-root and dividend-value lattice calculation has been removed.
This is checker consumption, not proof promotion: the reducer remains trusted
and its exact divide/remainder certificate routing is unchanged.

This forbidden-root checker accepts no proposition as authority and does not
turn its checked sufficient conclusion or selected bounds into a certificate
premise. `IntegerAffineBound` covers one mapped affine target bound, not this
correlated two-branch lattice conclusion. Producer selection of complete affine
root proofs/definition words is now live in the legacy reducer; a dedicated
correlated certificate conversion still remains. Until that conversion covers
every accepted family, neither exact row switches reconstruction or gains an
evidence-dependent fallback. No proof vocabulary, schema, or reducer node is
further promoted by the correlated custody form, and terminal closure remains
`fully-derived false`.

Proof-bundle v15 additionally carries exact fixed-integer `<=` endpoint
substitution. One recursively checked relation child, one recursively checked
equality child, and an endpoint index zero or one must reconstruct the exact
conclusion while leaving the other endpoint unchanged. This permits a future
untrusted producer to transport a closed literal bound across the preceding
SSA equality for that literal. The four canonical nonzero pilots consume that
capability; no reducer or operation row is promoted by doing so.

The production verifier now reconstructs settled scalar kernel questions
directly from `CanonicalScalarGoal`. `NonzeroDivisor`,
`ExactDivisionDefined`, and `ExactShiftCount` never invoke the mirrored
candidate selector: alternate available facts cannot change their question,
and the proof kernel checks only the producer-serialized derivation. The
mirrored selector roots are retained solely for compatibility tests on this
slice. Exact-cast representability, exact shift-left representability, and
exact add/subtract/multiply representability still have no language-settled
kernel proposition; they retain the legacy reducer pending the proposition
vocabulary decision recorded in `OWNER_QUESTIONS.md`. This is an explicit
remaining trusted dependency, not permission for new verifier search.

That producer status is also the module boundary. Structural Unit-plan
construction must not accumulate every sufficient-form recognizer merely
because it invokes them. Shared Boolean/integer convergence has a small
orchestration module over separate cast-chain, affine, product/divisor, and
shift/cross-family producer modules; its focused tests likewise separate chain,
affine-join, and nominal-cleanup responsibilities. Exact binary and cast
families are ordered declarative classifier registries consumed by one generic
dispatch path. Structural Unit planning likewise separates return analysis,
control/boundary construction, cleanup, call closure, and type/shape custody
behind a small orchestrator. The checked-to-terminal producer follows the same
boundary: its shared runtime-parameter classifier is a small orchestrator over
Boolean, conversion, affine, product/divisor, and shift/cross-family modules.
The six exact binary cohorts and exact-cast cohort select named ordered
registries through generic dispatch rather than repeating family permutations
inside the crate root. Structural scalar-return custody likewise has one
dedicated lowering/orchestration module whose nominal-cleanup specialization is
isolated behind the same entry point and whose expression-shape validation is a
separate subordinate responsibility. Structural Unit control has its own module;
structural Unit cleanup has one nominal family entry point over isolated ordered-
nominal and partial-affine implementations. Attached Unit closure assembly is a
separate orchestrator over provider discovery, exact call-closure custody,
type/domain/service publication, and parameter-transfer modules. General
structural-result transfer and result-bearing boundary custody have distinct
modules over one shared structural-type retention responsibility. Scalar-graph
terminal-module assembly is likewise isolated behind one parent-facing builder
and shares none of those paths. Content conservation, identity reshuffling, and
partition composition likewise share one lowering module whose three public
APIs retain their existing crate-root contract. Root-level producer regressions
are a small shared-fixture parent over isolated Unit-cleanup, scalar-graph,
content-ledger, structural-control, attached-Unit, and structural-return
families rather than a second responsibility embedded in the production root.
Proposition vocabulary, evidence-term identity, contract lanes, proof-output
invocations, and producer provenance likewise share one evidence-publication
module behind a single parent-facing installation API.
native machine emission keeps its byte/width/policy regression corpus
in the separately compiled `omega-machine-emission/src/tests.rs`;
the production root does not embed that second responsibility.
`omega-machine-emission/src/unit.rs` owns Unit-body and calling-policy
emission, exact per-target parameter homes, aggregate argument staging/copying,
and Unit stack/fuel/effect evidence behind the parent-facing Unit emitter and
cleanup-call helpers. `omega-machine-emission/src/cleanup.rs` owns
scalar-return and Boolean-control cleanup emission, nominal-cleanup admission,
exact residual partitioning, and cleanup stack/fuel/call evidence behind five
parent-facing contracts. `omega-machine-emission/src/scalar.rs` is a
small orchestration/re-export root over `scalar/x86_64.rs`,
`scalar/aarch64.rs`, and `scalar/shared.rs`. The architecture modules own their
scalar control, calls, arithmetic, register/stack mechanics, and exact byte
encoding; the shared module owns admissible conditional shapes, shared
convergence, integer-domain checks, and scalar stack evidence. The crate root
only orchestrates those responsibilities and retains shared ABI primitives plus
the public error vocabulary. These responsibilities must not be collapsed into
test helpers or one generic permutation dispatcher.
Scalar and structural crash routes, checked crash-site/frontier custody,
argument-root substitution, and canonical proposition construction likewise
share one crash/proposition module with an explicit internal contract surface.
Terminal operation emission and proof finalization likewise share one module;
they do not own scalar-graph classification or debug-map presentation.
Short-circuit Boolean decision/control emission and replaceable debug-map
presentation are distinct modules with distinct consumers.
Scalar-graph preparation, validation, partial evaluation, and expression
lowering likewise form one responsibility behind fourteen explicit internal
contracts; orchestration and public result assembly remain in the crate root.
Reachable scalar-call discovery and multi-machine terminal assembly are a
separate responsibility behind two explicit entry points.
The verifier reconstruction corpus follows the same ownership boundary: its
former 9,239-line sufficient-form test parent is a 15-line root over fifteen
cast, conversion, add/subtract, multiply/affine, join, shift, and divide-policy
modules. All 76 cases remain, and no family module exceeds 1,248 lines.
Terminal-module validation likewise has a 282-line parent, down from 7,498,
over separate structural/service foundation (956 lines), structural/boundary
operation custody (822), public error vocabulary (803), structural
ownership/frontier cleanup (750), per-machine registration/orchestration (716),
scalar crash/frontier and Boolean-predicate custody (674),
content-conservation validation/replay (534), operation operand/type custody
(522), partial/nominal affine cleanup custody (473), evidence/proposition
custody (410), control-flow/dominance validation (301), proposition-root
projection (146), contract proposition scope (120), and call-graph acyclicity
(68) modules. The public validation vocabulary remains re-exported at the crate
boundary.
Additional sufficient-form families should follow that shape: one closed
responsibility per module, explicit
precedence at the registry, and no authority beyond the certificate proved
against the unchanged canonical ledger. This refactoring rule does not make the
current Rust producer or its registries trusted.

The exact-shift sufficient-form family now applies that boundary internally.
One small parent owns exact-left dispatch precedence and the primitive shift
obligation, one direct-chain module owns landed counts, interval transfer, and
homogeneous/mixed chain foundations, and one composition module owns
shift/cast/affine/divide cross-family reductions. Their public reducer contract
is unchanged. The executable integer-shift trust node hashes all three source
files, so modularization cannot move deciding bytes outside its migration
custody.

Exact conversion follows the same boundary. Its parent owns cast-family
precedence and the direct range fallback, its chain module owns conversion
spines and interval transfer, and its composition module owns divide/product/
affine/offset cross-family reducers. The conversion trust node likewise binds
all three source files, while the parent-facing reducer contracts and accepted
algebra remain unchanged.

Premise availability is path- and version-sensitive. Ranking ledger nodes makes
cyclic justification unrepresentable but does not by itself make a fact
available. A cited evidence token must dominate its use and remain valid along
every path. At an acyclic join, a new merge token may be established only from
matching valid tokens on every predecessor. A fact from one arm alone is not
available after the join. Cyclic reconvergence never uses that merge rule: it
requires a separately checked invariant-establishment and preservation
derivation. A partial operation establishes its result equation only on its
normal successor and only after its own safety obligation, so that equation
cannot prove the operation that creates it.

Calls have two independently checked responsibilities. Coverage emits one exact
obligation identity for every callee `requires` clause. Instantiation then
checks arity, binder kinds and types, capture-free positional substitution,
pre/post state versions, moves and reborrows, outcome guards, crash routes, and
evidence lifetime. Forgetting a clause and substituting the wrong caller term
are separate unsound failure modes.

Artifact acceptance ultimately composes two theorem families:

```text
safety / partial correctness
    exhaustive derivation + sound rows + valid premises + checked obligations
    => no execution prefix violates the selected safety policy and every
       completed outcome satisfies its contract

progress / total correctness
    well-founded orders + per-edge descent + complete SCC/call closure
    + accepted environmental progress premises
    => every published termination guarantee holds
```

Logical fuel discharges neither theorem. Exhaustion is sponsor-owned suspension
at the unpaid site followed by resume; it is scheduling and attribution, not a
termination argument.

Each schema soundness theorem is universally quantified metatheory proved once
in the low-rung calculus, not a quantified proposition repeated in an artifact
certificate. It cites exact digests for the schema row, shared state model,
mathematical definitions, operational clauses, and generic composition theorem.
A checked conservative-extension theorem may transport an unaffected row to a
new semantics version; otherwise the row is reproved. Old artifacts retain
their pinned semantics identity while that version remains supported.

The executable trust ledger is a closed dependency graph, never an informal
label. Every dependency must terminate at an explicitly registered root with
kind, semantic subject, digest/version, owner, scope, rationale, and accepting
policy; unknown or cyclic leaves reject. Until proofs replace them, the current
Rust generator, each uncertified reduction family, the low ledger framework,
each unproved leaf-denotation row, and each unproved call-composition row appear
as distinct trusted-judgment dependencies. A leaf row may move from trusted, to
a locally checked row theorem, to inclusion in the checked module-composition
bridge without hiding the remaining global dependency. Call rows move only
after their separate coverage, substitution, outcome, crash-route, and evidence-
lifetime composition obligations are established.

The current Rust migration surface is now exposed by `psi-terminal-codec` in
every verified proof synopsis. Its validated graph binds exact source bytes and
explicit versions for the decoder, proof kernel, verifier, each sufficient-form
reducer, the unproved ledger framework, 32 scalar-denotation rows, four
separate structural/effect rows, and four call-composition rows covering every
closed `OperationKind`. The 36 leaf / four call operation-custody split remains
unchanged. Dependency edges
contribute to the graph identity; unknown, cyclic, unreachable, duplicate, or
noncanonical custody rejects. The entry deliberately reports `fully-derived
false`: this inventory is the prerequisite for, not an implementation of, the
low canonical ledger.

The production structural/effect table extends the closed-row schema:
`EstablishByteSequenceLiteral`, `BooleanStructuralField`, `PortWrite`, and
`EstablishTrivialAffineLocal` expose
their result, exact custody, action, external-effect, one-fuel, and place-
frontier policies through one exact-unique Rust table. Its generic interpreter
keeps the Boolean equation, observable port effect, and affine establishment
event distinct. Verification consumes the shared Boolean equation rather than
maintaining another operation-specific reconstruction arm. Trust identities
also bind the modular verifier's evidence-provenance, integer-foundation,
proof-bundle, reconstruction, and substitution sources, so splitting the former
monolith does not weaken exact deciding-byte custody.

Call composition follows the same production shape without pretending that a
call is a primitive denotation. One exact-unique four-row table independently
declares target, result, positional arguments, requirement handling, structural
transfer, successful outcome, crash routes, evidence lifetime, fuel, and
frontier policy for `Call`, `CallUnit`, `CallStructuralScalar`, and
`BoundaryCall`. Execution-grade
module validation remains responsible for proving each concrete signature,
state/movement, clause coverage, capture-free substitution, transfer, outcome,
crash, and evidence invariant. Only then does the focused call-composition
module enumerate obligations and import successful guarantees. The general
operation reconstruction loop no longer owns parallel call algorithms,
and each call trust node binds both the shared policy table and that focused
consumer.

The historical Gamma feasibility spike proved that exact canonical-byte
decoding and ordered ledger reconstruction can fit the low rung without making
the Rust verifier authoritative. At its final measured checkpoint it assembled
to 4,982 typed Gamma lines / 198,971 bytes / 423 functions with maximum source
nesting 25. Its closed row tables eliminated per-operation builder branches;
most remaining repetition came from Gamma's monomorphic decoder-result types,
not the semantic schema. That format-bound implementation was retired when its
format-18/vocabulary-20 decoder fell behind the then-live
format-22/vocabulary-25 artifact. Git commit `a5cfd83cc` and its follow-ups
retain the executable
provenance; dead source is not carried as a parallel verifier.

The reusable result now lives in production's closed 40-row inventory: 32
scalar denotations plus four structural/effect leaves form 36 leaf rows, while
four call-composition rows remain a separate algebra. Exact-unique lookup and
mutation tests retain the schema discipline. Reusable low-rung byte,
scalar/type/value, UTF-8, and structural-leaf grammar fragments remain gated,
but deliberately claim neither a fixed terminal header nor a complete live
decoder. The full assurance-owned low generator, row proofs, and composition
bridges remain required; the retired feasibility result marks no trust-graph
dependency derived.

Portable terminal-Psi denotation bottoms out in the abstract terminal execution
model, not in x86-64 or AArch64 behavior. ISA semantics, hardware fidelity,
native lowering, and installation belong to the separate native-refinement
closure. A future Psi-hosted proof-kernel implementation may accelerate or
independently cross-check certificate validation, but it produces no semantic
ledger and therefore supplies no reconstruction assurance.

## Canonical semantic bytes

`psi-terminal-codec` owns one canonical encoding of the supported in-memory
vocabulary. `PSITERM\0` bytes carry a format marker and the one current-vocabulary
marker. They use fixed-width little-endian counts, stable nonzero identities,
full-width integer payloads, and closed sum tags. The format favors auditability
over density.

Unordered semantic sets are strictly sorted by stable identity or canonical
bytes and reject duplicates; symmetric terms use the same canonical ordering.
Execution-significant sequences—parameters, operations, and jump arguments—
retain declaration order. Recursive terms have a fixed depth limit.

Decoding fails on stale markers, unknown tags, invalid identities or values,
noncanonical forms, verifier-invalid modules, truncation, or trailing bytes. A
successful decode re-encodes byte-for-byte; the decoder never normalizes an
alternate spelling. `TerminalPsiIdentity` binds a domain-separated hash of the
exact canonical semantic bytes and excludes replaceable proofs, installation
records, and debug maps.

Operation variants are closed, typed, and refer only to already defined values.
Each variant reconstructs its logical result and obligations under the
vertical-slice rule above. This pre-release project has no format migration
path: semantic changes move the compiler, codec, verifier, interpreter, and
lowerers together; stale modules reject. Golden tests freeze only the current
encoding and identity.

Proof bundles have separate canonical `PSIPRF` bytes and identity. They carry
one current proof-system marker; stale markers reject. Evidence is strictly
ordered by obligation identity and retains exact kernel rules, proof trees, and
admission identities. Proof propositions preserve rule direction because cited
axiom direction is significant even though the proof section is replaceable.
Disjunction introduction records exactly one independently checked child and
its selected canonical arm index; an absent arm, out-of-range index, or child
whose conclusion differs from that arm rejects. This is proof-calculus
capability only and does not promote any semantic-ledger row or sufficient-form
reducer.

`TerminalArtifactManifest` binds semantic and proof identities plus optional
installation and debug hashes. Each role has a separate hash domain, and absent
differs from present-but-empty. Replacing a valid nonsemantic section preserves
`TerminalPsiIdentity` while changing its own section and container identities.

The canonical `PSIINST\0` installation payload binds semantic identity, target
facts, exact profile/provider decisions, the complete emitted-image hash, and
text-validation evidence. Format 42 carries separate domain-framed SHA-256
digests for encoded compiler text, final compiler text, the canonical
relocation envelope, and their derivation; retained compact text fingerprints
are report compatibility only. It is manifest metadata, not executable
authority; installation still consumes separate admission and placement
authority. Debug maps are replaceable presentation metadata bound to the exact
semantic identity and never participate in semantic meaning.

For effectful Unit roots, the payload also retains the canonical function map,
each privileged port effect's exact service/operation/byte range, and each
bodyless settlement's exact admitted provider-execution binding and immediately
preceding effect realization. A settlement emits no duplicate hardware effect;
object and installation validation reject missing, reordered, byte-drifted, or
raw-number-only realizations. Production construction consumes the same
ledger-owned `ProviderExecution` values used by target lowering and requires
their closure to match the emitted settlements exactly; decoded payloads remain
non-authoritative audit projections. Both native lanes stage structural
parameters into aligned owned entry homes before effects or calls. AAPCS64 Unit
calls preserve `x30`, keep `sp` 16-byte aligned, marshal direct register and
stack fragments, and create the normalized caller copy for indirect by-value
aggregates before passing its address.

Native Unit artifacts and the canonical installation payload retain one
logical-fuel attribution row for every emitted operation and return edge:
exact current schedule, semantic site, units, operation ordinal,
function-relative byte offset, and byte count.
Metadata-only settlement rows deliberately have a zero-byte interval. This is
the provenance input to sponsor-owned inserted metering when installation
selects a dynamic native realization; the row is not evidence that runtime
charging already occurs and is not a native instruction-cost model.

## Logical fuel

`psi-terminal-fuel` owns accounting identity independently from terminal
semantic identity. The schedule exhaustively assigns cost to every closed
operation and terminator variant, so extending the vocabulary requires an
explicit accounting decision. A schedule change never changes program identity.

The interpreter charges before each semantic site and reports deterministic
total and per-`OperationId`/`EdgeId` usage. Fuel is sponsor-owned: exhaustion
is not visible or catchable by the Psi machine. Resumption continues at the
unpaid site without replay or double charging; a completed crash edge is not
charged again.

The maximum logical work of an entry or segment is the greatest sum of charged
units along any one admitted path. Sequential operations and calls add; mutually
exclusive branches take their maximum. This differs from simultaneous stack
use, where sequential callees normally compose by maximum. Certificates and
reports use fuel units and do not claim native instruction count or time.

Native fuel is accounted per sponsor region. Same-sponsor calls share one
private activation context; a separately sponsored edge begins another region.
Fixed provision suppresses the meter only when the exact installed certificate
fits the region's grant. Dynamic lowering compares the remaining allowance
with the site's required units before subtracting or executing. Paying exactly
to zero succeeds. A failed comparison changes nothing and transfers the exact
schedule, unpaid site, required units, and remaining units to the sponsor.

That transfer is architectural suspension, not a Psi safe point. Opaque native
register/stack/program-counter state is the only continuation and is never a
Psi or Omega value. Resume restores it at the failed pre-charge check; fuel
exhaustion alone authorizes no semantic cleanup, cancellation, migration, or
replacement. The target transfer stub is unmetered compiler/runtime machinery;
authored sponsor policy, when present, executes in a separately and completely
fixed-provisioned region.

`FuelSuspensionFree` is a transitive installed-root fact rather than a property
of one local certificate. Transparent closures derive it only when every
reachable sponsor region is non-exhausting. Opaque provider summaries publish
work units and suspension behavior as independent admitted facts; absent
suspension evidence fails the derivation.

`psi-terminal-fixed-fuel` derives certificates from verified terminal control.
For acyclic control and call graphs it computes the greatest entry-to-exit path,
taking the maximum rather than the sum at exclusive branches and including the
outcome-specific bound of each reached callee. A callee crash does not acquire
the cost of an unreachable caller tail; a caller segment ending after a call
uses only the callee's normal-return bound. Entry and segment certificates bind
the exact terminal identity, schedule, endpoints, and ceiling; validation
reconstructs those fields and the complete reachable segment partition. Ranked
control has one additional whole-entry slice: an opaque fixed-fuel verifier
carrier accepts only the exact one-machine unsigned countdown, and the checker
recomputes `preheader + (upper_bound - lower_bound) * (header + decrement) +
(header + exit)`. Under the current schedule the `u32` source countdown costs
`5 + 6n` and has all-input ceiling `25_769_803_775`. Arithmetic and final `u64`
conversion are checked; an unrepresentable ceiling rejects. Ranked safe-point
segments, tail calls, and relevant-precondition refinements require later
vertical slices.

Omega may use a certificate only for the exact installed terminal bytes,
architecture, entry stub, and external-root context it names. Recomputable Psi
fuel evidence carries no provider receipt.

`omega inspect-terminal --machine <qualified>` verifies the selected terminal
closure and proof bundle, recomputes and validates its acyclic entry
certificate, and publishes the exact terminal identity, schedule, entry, and
ceiling. This is build-time semantic evidence, not installed-root evidence:
the native terminal Unit and scalar slices retain exact emitter evidence that
object construction replays into local peaks, caller-live bytes at typed calls,
and an acyclic closure demand. Accountable acyclic scalar conditionals use one
depth-independent carrier: physically ordered decisions, a true-before-false
DFS return/crash bitmap, and one ordered x86 division-diamond ledger. Object
construction reconstructs every exact prefix and leaf, validates each branch
and terminal crash encoding, partitions division diamonds by region, and takes
the maximum across exclusive paths. AArch64 and branch-free x86 expressions
reuse linear replay; signed x86 wrapping/saturating division retains its exact
special/ordinary diamonds. The same facts survive typed call arguments,
relocations, object/image validation, installation serialization, and installed
closure recomposition.

Boolean parameter/expression conditionals retain the same tree and call-stack
evidence. If terminal lowering source-distributes one semantic Psi convergence
call into several leaves, the object boundary permits its repeated operation
owner only when every physical pair has conflicting outcomes at a validated
decision. Calls sharing an executable path still reject. This proves the
source-distributed tree, not an actual reconvergent native join. Separately, the
finite runtime-Boolean-parameter tree slice retains one terminal-Psi join and
object replay validates its ordered native decisions, non-final-leaf
unconditional join branches, final-leaf fallthrough, and single cleanup tail on
every target. General shared native control-flow joins remain outside the
theorem. Affine cleanup admits the finite
branch-only trees described above, with one distinct cleanup-bearing return
edge per surviving leaf. The shared form also accepts Boolean equality against
a constant: Psi normalizes that leaf to identity or negation before emitting
the existing convergence carrier, so no comparison operation crosses the
terminal boundary. It additionally accepts one direct relevant Boolean field
identity on one claim-free affine nominal-cleanup root; the terminal operation,
verifier, interpreter, fuel model, codec, and every native target retain the
exact source place and canonical field ID. At least one Boolean parameter keeps
that source outside native expression scratch. The bounded direct-integer-
comparison form specified in the nominal-cleanup section above retains the same
exact Psi operations, contract premises, and certificates through this
verified, interpreted, and native shared join. The operation list and accepted
proof shapes are stated once there; this native accounting section adds no
second vocabulary.
Field-only trees,
nested or multiple member identities, wider or partial integer computation,
member/comparison mixtures, external adapter/architectural-arrival state, and
other terminal function forms remain outside the shared-join theorem, so the
inspection surface makes no installed-root WCSU claim. External-root admission
must join the emitted body demand with the separate context-indexed entry-epoch
realization; it must never relabel target or opaque-provider arrival evidence as
Terminal-Psi derivation.

## Implementation queue

[`TASKS.md`](../../../TASKS.md) owns remaining terminal-Psi work. Temporary
differential paths may coexist as test oracles while consumers move; they are
not alternate language versions or a permanent Omega-to-Psi path.
