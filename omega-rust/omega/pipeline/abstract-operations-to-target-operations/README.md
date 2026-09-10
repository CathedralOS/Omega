# Abstract operations to target operations

This stage derives target operations and call placement from abstract operations.
Start at [lib.rs](src/lib.rs). The public reference contract is
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md).

## Target projection and independent coverage

This stage translates abstract operations using the exact target, selected
mechanisms and ABI/layout owners. It preserves operation order, source values,
ownership and boundary occurrence identity. It does not authorize effects,
discharge source borrow/proof obligations, assign physical registers or choose
result-home offsets. Concrete assignments belong downstream. A normalized foreign
locator's sealed target profile owns applicability; this stage checks that target
rather than maintaining another format/case allowlist.

[Validation](src/validation/mod.rs) rejoins the target, semantic entry and complete
function roster, then uses the ordered family catalog to select independent
translation replay. Ambiguous classification rejects. A receipt names the exact
function families covered; whole-plan root/roster custody is not proof of semantic
translation for every unmatched function. Extend both source classification and
independent replay when admitting a family; a producer-only variant is not closure.
The [target representation](../../representations/target-operations/src/target_operations.rs)
owns resulting control, value, storage, call and boundary data.

Exact numeric replay retains source definitions, operand order, distinct source
and destination types, ABI locations and proof obligations. Wrapping shifts keep
independently typed counts and modulo-width meaning; exact shifts retain their
count-range and, for left shift, result-representability obligations. Native
integer widening must validate its exact sign/width relation, not just copy bits.

Ordered Unit bodies retain pure scalar definitions as `ScalarDefinition` with
an exact `TargetScalarExpression` and value-residence requirement. Integer
widening uses the same total sign/width relation as scalar functions. Earlier
computed values are referenced by `ScalarHome`, never by duplicating their
producer or manufacturing an ABI parameter. The requirement identifies the
operation, value, type and shape; register allocation owns physical residence,
and target lowering does not mandate a stack slot. Native realization may
support a narrower set of widening shapes than this target-level vocabulary.
FMA target records retain occurrence, selected plan and admission without choosing
XMM homes. An available target record does not imply that the common downstream
selection/emission path supports it.

Ordinary Unit control, cyclic integer-result functions, and IEEE scalar transport
and comparison functions use target-owned blocks
and explicit `Jump`, `Conditional`, `StructuralCase`, `Return`, and
`ReturnScalar` terminators in `TargetControlGraph`.
Nonterminal definitions and calls reuse the ordered Unit operation vocabulary;
branch targets are block identities, not nonreturning-arm layout ordinals.
Invocation borrows retain the ordinary signature owner's referent layout and
placement throughout the graph. Field stores reuse ordered Unit store lowering;
loop-carried integer sources retain exact block/value/type coordinates. This
does not add transferred record descriptors, owned cleanup, or qualification
support to graph edges. A prepared invocation-place lookup is shared by graph
operations rather than rebuilt for each store or call.
Start at [control_flow.rs](src/lowering/control_flow.rs): signature preparation,
dominance, operations, terminators, and edge bindings have separate owners.
Lowering retains dominating definitions and authored block order independently
of traversal. Actual topology selects the cyclic scalar route, not a ranking
annotation. Scalar bodies with primitive writes also use this graph to retain
the store before their scalar return; other acyclic scalar functions retain
their existing expression lowering.
The common target-to-selected
reader independently checks this graph against source before constructing the
existing legalized/selected graph. The target-only family receipt does not claim
this coverage; its legacy continuation check still rejects linear graph sources
that match that older family. Scalar block arguments retain their destination
block, value identity and type, independently of function ABI parameters and
operation-result homes. Each edge preserves the complete ordered bindings;
joins introduce destination-owned values rather than substituting one arrival's
expression. The existing selected graph retains these as parallel transfers.
Selected edge bridges snapshot arguments before destination replacement.
Ordinary allocation realizes those copies even when incoming and destination
values cannot share a home, including simultaneous backedge swaps.
Structural byte-view block parameters and ordered bindings retain their exact
declarations through target and legalized graphs. Destination views have their
own block/place identity, not an inherited producer operation or length value.
The selected consumer must realize their descriptor transfers; this upper
projection alone grants no native transport or cyclic admission.
The node set includes scalar constants, integer widening, exact integer add and
subtract, immutable byte length and read observations, integer and IEEE comparisons,
subslice establishments, scalar calls and ordinary Unit calls. Arithmetic retains
its exact safety obligation; scalar calls retain their callee ABI and result home
without requiring a fabricated caller attachment.
IEEE comparisons retain their relation, format, ordered operands and Boolean
result home. Float parameters, call results, returns and block arrivals retain
their IEEE scalar type; raw-bit transport is not a conversion to integer language
semantics. The selected provider and exact authored application remain in the
compiler's checked coverage companion, independently of this numeric operation.
Read/subslice views must be exact shared parameters or dominating establishments;
their length-observation identity is retained separately from scalar residence.
Whole mutable machine/block parameters support length observations and indexed
byte writes. A write retains the exact declaration, original descriptor, typed
index/byte sources, current length and bounds obligation in the ordinary Unit
graph. It neither changes descriptor extent nor creates a mutable subslice.
Comparisons define Boolean homes for conditions rather than repeating their
producer at the edge. Boolean parameters and constants can cross block edges;
computed comparison results still require native value materialization before
they can be transferred or passed as Unit-call arguments.
Abstract lowering preserves entry parameter metadata that exactly repeats the
function parameters, but this is not native admission: optimization-unit
validation still requires empty entry block parameters.
Legacy cleanup/provider templates are not reinterpreted as ordinary graph edges.

Natural-ranked and unranked scalar cycles use the same native physical sequence.
Verified source custody preserves their exact graph; unranked execution acquires
no termination certificate or fixed-work bound. The
[source-produced scalar cycles](../../../../tests/native-differential/tests/scalar_control_cycles.rs)
exercise selected calls, scalar returns, and simultaneous backedge swaps through
object/image/installation replay on four hosted targets and execution on supported
hosts. Whole plain-owned, unqualified, claim-free Affine or Unrestricted inputs
can also cross exact block arrivals when no runtime operation observes those
owned places. Their full value ABI, declarations, ordered bindings, and
no-code affine return discards remain in the graph. Scalar-only calls remain
ordinary executable operations. Current ownership validates the exact live
frontier and disposal order; target availability supplies no ownership authority.
The input-only selected reader independently checks this bounded route and its
ABI. Runtime observation of owned machine inputs, projected or structural call
actuals of those inputs, and executable cleanup require separate native realization.
Independently established primitive locals can be read, written, and borrowed
while those owned inputs remain unobserved.
The [owned control-cycle regressions](../../../../tests/native-differential/tests/owned_control_cycles.rs)
exercise ranking-only field erasure, selected scalar calls, and simultaneous
owned swaps through four-target publication and matching-host execution.

Plain owned scalar-sum results also cross ordinary state arguments before case
dispatch. Each destination retains its exact block parameter declaration and
sum layout in a block-owned home. Different predecessors can supply distinct
results, and an intermediate state can forward its own parameter. A destination
cannot alias one predecessor's result home: joins must work whichever edge runs.
Availability follows dominance and exact ordered bindings; current ownership
still decides whether an affine source may move or be discarded. Operation-only
call and image records continue to require an actual operation-result origin.
The [owned-state regressions](../../../../tests/native-differential/tests/scalar_case_results/owned_state.rs)
retain full-width payloads and borrowed output storage through publication on
the three direct aggregate targets and execution on a matching supported host.
Tag-only sums also publish on Windows x86-64; this cross-target check does not
claim Windows execution from a macOS host.

Scalar-returning primitive-store callees retain the same exclusive reference
parameter, ordered store, and mixed scalar/structural ABI as their source.
Boolean and fixed-integer referents retain their exact borrowed width; scalar
results do not change reference identity. IEEE referents remain outside this
scalar-returning store admission.
The common graph reuses Unit primitive-store lowering; it does not wrap the
callee in a Unit function or convert its referent to an owned copy.
The [source-produced store-return tests](../../../../tests/native-differential/tests/primitive_store_return.rs)
exercise the original caller word and independent scalar result through the
ordinary physical and publication route.

Primitive-local establishment, replacement and fresh reads use ordered graph
operations with exact producer/place and scalar-value identities. Establishment
requires activation-local backing; it is not an incoming ABI parameter or scalar
snapshot. Borrowed calls retain an `EstablishedPrimitiveLocal` argument origin
and their real scalar result. The selected consumer derives the local address
and replays initialization, exact-width writes, and subsequent loads. The
[primitive-local regressions](../../../../tests/native-differential/tests/primitive_locals.rs)
cover the unchanged ranked `walk`, direct replacement and loop reinitialization
through native publication. Fixed 8/16/32/64-bit signed and unsigned integers,
Boolean, and IEEE binary32/binary64 retain exact-width reads and writes. IEEE
homes carry raw payloads, not numeric conversions. Ordinary Unit helpers can
replace a local while an earlier scalar snapshot remains distinct from its next
read; readable primitive references can initialize that local.

Closed-sum inspection of an admitted boundary result uses this same graph,
without a fixed block count, arm order, or exit-only body template. Each case
retains its declared tag, target block, exact relevant integer field offsets,
destination block parameters, and authored affine discards. Payload parameters
are edge-produced values, not scalar results attributed to the read operation.
Returning arms, ordinary joins, repeated field projections, and later dispatch
before cleanup retain their source topology. The structural-home lookup proves
definition availability, not ownership liveness; validated abstract frontiers
remain authoritative. Independent target-input replay checks producer/home,
layout, case, payload and cleanup correspondence. The selected byte-read case
route also retains returning arms with fresh affine read results. Their no-code
discards remain on the exact return edge; validated current ownership determines
which results remain live, independently of home availability. Mandatory fragment
replay carries that complete graph through object/image publication without a
redundant singular cleanup projection. Hosted
exit arms retain every nominal Unit return; the canonical process-exit migration
remains separate.

Dynamic-descriptor lowering preserves establishment, rebinding, aggregate store,
and parameter forwarding as distinct semantic sources. Retain the exact selected
application, receiver projection, requirement and native call plans, not a
descriptor reconstructed from an ordinal or an indirect call replaced by a direct
one. Transparent helpers preserve the incoming two-word descriptor and its exact
outgoing ABI; Unit calls carry no fabricated scalar result. Boolean results retain
Boolean control rather than an invented integer comparison. Mutable projections
retain each path segment and checked offset for independent physical replay.
Object/image/installation joins must bind table, adapter, realization, relocation,
stack and code-span custody. Historical target-family support does not establish
current native execution; unsupported common-route transports remain fenced.

## Structural ABI derivation

[structural_signature.rs](src/lowering/structural_signature.rs) constructs
structural call signatures from declarations and resolved referent layouts.
The exhaustive access classifier in
[structural_layout.rs](src/lowering/structural_layout.rs) selects value placement
only for owned inputs. Shared, mutable, and write-only borrows use
`BorrowedReference`, preserving referent size/alignment and passing its pointer
without a caller-side value copy. Layout caches retain referent shapes, not
access-specific parameter shapes. Scalar and erased descriptor ABIs retain their
existing generic construction.

The native byte-observation path admits one unqualified, unrestricted shared
`BorrowedView` parameter, `u64` scalar inputs, and a `u64` result. It uses
`BorrowedReference(16, 8)`:
one pointer to the original two-word descriptor, with the length at byte offset
eight. The selected `Load64` retains the logical source place and independently
replayed read footprint. A guarded `ByteSequenceRead` loads the backing pointer
at offset zero, then performs an indexed, zero-extending byte load. It retains
the exact index, same-view length witness, and accepted bounds obligation.
The ordinary conditional graph preserves the non-reading branch; shared
[byte observations](src/lowering/scalar/byte_views.rs) serve both straight-line
and integer-result conditional lowering. Scalar inputs precede the descriptor
pointer in the derived call signature.

Scalar-result helpers can forward one whole shared descriptor and return the
callee's `u64` result, including through nested helper calls. The existing
`ReturnStructuralScalarCall` target form enters ordinary scalar graph legalization;
[call input checks](../target-operations-to-selected-instructions/src/legalization/scalar_graph_input/structural_call.rs)
rejoin the exact source argument, callee declaration, reference ABI, and placement.
Selection snapshots only the pointer, never the descriptor's contents, and
independent replay checks its incoming and outgoing homes. No separate call IR
or byte-view calling convention is introduced.

Mixed scalar/view calls also use ordinary `StructuralCall` integer expressions
in straight-line and conditional graphs. Each occurrence retains the exact
callee plan, ordered scalar actuals, whole-reference structural actuals, and
call contracts. Repeated calls remain ordered operations even when an earlier
result is unused. This route admits unqualified, unrestricted shared byte-view
parameters; it does not infer projection or mutable-reference support.
Standalone selection and replay also rejoin the caller's complete scalar roster
and placements to its ABI before forwarding the descriptor pointer. An extra or
missing scalar declaration cannot be inferred from an otherwise plausible ABI.

Straight-line Unit bodies retain repeated whole shared-view Unit calls with
Boolean/integer parameters or literals through the existing `UnitBody` operations
and structural signature classifier. A Unit helper may invoke a scalar byte-view
helper and discard that scalar result before `ReturnUnit`; the enclosing Unit
signature has no result placement or fabricated scalar result. Whole references
retain their original pointer placements. Native selection admits `u64` and
Boolean scalar actuals beside one whole shared view, with resultless call
constraints for each supported native register arity. Independent replay checks
the scalar types, original pointer, call order, and absence of a result operand.
The ordinary Unit graph above also admits byte-dependent acyclic branches.
Descriptor rebinding across block parameters and the console writer's ranked
loop remain separate dependencies.

Checked subslices retain their exact source, structural result, endpoints, and
two-leg bounds obligation. Acyclic scalar-result graphs can measure, read, and
derive nested views without copying the original descriptor or backing bytes.
Target expressions retain the derivation, and legalization independently rejoins
it to the verified operations. Derived views need an addressable descriptor
before they can be passed through the reference ABI. Ordinary scalar-result
and Unit calls reuse the selected activation-local descriptor storage; the
descriptor points into the original backing rather than copying its bytes.

Selection retains the original backing pointer with an integer byte offset and
length in ordinary value homes. For root length `R`, each derived view preserves
`offset + length <= R`; the two bounds legs justify nested offset addition and
length subtraction. Guarded reads add the relative index to that offset before
the existing indexed load. No arithmetic proof is asserted about the pointer,
and an empty suffix need not form a one-past address. Independent replay checks
the same source chain and every contributing home.

This does not complete literal descriptor materialization, computed Boolean
Unit-call arguments, derived-view block transfers, or ranked control. The
[native regression](../../../../tests/native-differential/tests/terminal_byte_views.rs)
starts from encoded, verified Terminal, cross-lowers four hosted targets, and
executes caller-owned descriptors, derived views, and framed helper chains
on supported hosts; cross-emission alone does not establish matching-host
execution or Omega-source writer closure.
The void-call runtime fixture checks normal return, descriptor preservation, and
surrounding stack canaries. Its discarded byte-reader result does not establish
observable byte output or Boolean-dependent behavior. The separate
`byte_output/derived_unit.rs` fixture retains a checked suffix, a genuine Unit
reader's guarded byte output, and caller continuation. It publishes ordinary
objects, Linux images, and installation records. Structural call records keep
incoming ABI placements distinct from established-view producers and their actual
local descriptor slots; mandatory retained replay binds the source and physical
transport. Installation format 89 encodes that distinction and rejects older
markers. The runtime oracle consumes validated published text and requires a
Linux host. Scalar-result view expression/call image publication and
literal-backed descriptors remain separate dependencies.

[Structural-header validation](src/validation/structural_signatures.rs) rejoins
retained Unit and mixed scalar ABI parameters to the source declarations and
reconstructs scalar-prefix, result, and structural placement. A coherently
recomputed value ABI cannot replace a source borrow. This check does not claim
full function-body translation coverage or native caller-visible behavior.

Finish caller preparation and independent native receiving/replay checks under
`STRUCTURAL-BORROW-IDENTITY` on the [execution board](../../../../TASKS.md).
Embedded callee plans and argument homes require their own reconciliation;
standalone native consumers cannot assume a producer ran header validation.
Existing owned-copy structural-call forms remain closed to borrowed execution;
the byte observations and scalar helper forwarding above preserve their exact
reference input contract.

A staged pointer and staged value bytes are different. A direct-home test is
valid for owned semantics, not evidence that a borrowed caller sees a write.
Test substituted access/shape/placement pairs as rejections and execute genuine
reference controls against caller-owned backing. Unsupported native forms remain
fenced until all producing and receiving boundaries enforce their contract.

## Store and argument replay

Lowering retains complete projected paths and scalar value sources. Physical
assignment and later replay must reconstruct offsets from declarations, match
incoming and outgoing placement, and preserve exact referent widths. A physical
pointer cannot erase write-only access or collapse Boolean/IEEE/integer stores.

Parallel argument transfers must preserve every original source despite register
cycles, duplicate sources, or register/stack crossings. Any private snapshots
store scalar sources or reference pointers, not borrowed referents. Call-result
sources use their exact durable homes; they are not fabricated constants.
Frame rebasing, transient call storage, relocation, and code intervals remain
independently replayed downstream.

Field-store/dynamic-call fixtures alone do not establish general borrow
writeback. Admission of a new native shape needs caller observation after return,
unchanged surrounding bytes, and suspension checks where applicable, in addition
to layout and byte replay.

Straight-line Unit primitive/field stores and projected exclusive Unit calls enter the
ordinary graph with original borrowed pointers. Integer stores use exact 1-, 2-,
4- or 8-byte footprints; Boolean stores use one byte and IEEE stores use their
exact 4- or 8-byte format. Selection independently
reconstructs each field/index projection and its byte offset from declarations.
`AddressOffset` adjusts the pointer without reading its referent or asserting
source arithmetic, and `Store` writes that referent rather than a frame slot.
Address work adds no logical charge; the authored store and call retain their
operation and fuel custody. The
[source-produced receiver tests](../../../../tests/native-differential/tests/terminal_psi_indexed_receivers.rs)
observe caller bytes, padding, interleaved projections and repeated calls on
supported hosts, and cross-emit the indexed alias for four hosted targets.
The same probe covers incoming/outgoing stack pointers and a retained root with
runtime scalar arguments across three calls. Its `publication::` group carries
that program through ordinary object/image/installation records, executes final
published text on the matching supported host, and rejoins spill-inclusive frame
demand. Borrowed argument records name the actual call instruction; retained
physical replay establishes pointer projection and transport without a referent
copy. Installation shape checks alone do not establish semantic path identity.
Run it with `cargo nextest run -p omega-native-differential-test --test
terminal_psi_indexed_receivers --no-fail-fast --no-tests fail`. Cross-publication
does not replace Linux native caller observations. Its `primitive_stores::` group
also checks whole primitive replacement through forwarded calls, signed and
unsigned runtime values, both Boolean values, literal stores with unused inputs,
and stack-passed primitive roots across three calls. Primitive declarations stay
primitive; the ordinary store retains exact source type and width without a
synthetic record or field. The `ieee_stores::` group checks runtime IEEE primitive
and projected-field replacement, mixed integer/f32/f64 register and stack
arguments across repeated Unit calls, and primitive literals with unused inputs.
Caller comparisons retain NaN payloads, signed zero, subnormals, and surrounding
bytes. The `ieee_literals::` group checks projected field literals and literal
Unit-call actuals, including mixed binary32/binary64 register and stack arguments.
Each literal source retains its preceding definition, SSA value, format, and raw
bits; receiving replay checks these independently before native materialization.
Computed floating sources and general control/cleanup remain native dependencies.
