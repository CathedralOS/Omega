# Abstract operations to target operations

This stage derives target operations and call placement from abstract operations.
Start at [lib.rs](src/lib.rs), then [function routing](src/lowering/function/mod.rs).
The target representation owns the resulting control, values, storage, calls,
and boundary data; this stage does not assign registers or frame offsets.

Provider, callback and authority admission belongs to
[stage 00](../00_terminal-psi-to-abstract-operations/src/native_admission.rs).
This stage consumes admitted settlements and independently checks their lowering;
it does not select providers or authorize execution.

## Functions have one route

Every function enters
[control-flow lowering](src/lowering/control_flow.rs). It requires explicit source
blocks and retains definitions, calls, branches, and returns in
`TargetFunction.graph`. Unit, scalar, and aggregate returns are graph terminators,
not alternative function representations. Missing block structure rejects. There is no flat-input
compatibility adapter, expression-tree function planner, special-form fallback,
or test-only entrypoint to one.

Each computed scalar has one result home. Uses reference that home instead of
copying its producer tree or repeating a call. Constants and operation-local
expressions retain their source identities and arithmetic obligations; they are
not a second whole-function representation. Block parameters belong to their
destination block, and edges retain complete ordered transfers.

Signature preparation, dominance, operation lowering, terminators, and edge
bindings have separate owners inside the common graph. Unsupported operations,
qualification transport, cleanup, or ABI relationships reject at their owning
check. Supporting them means extending the ordinary operation/transfer contract,
not restoring a whole-body recognizer. Scalar native kernel coverage can be
narrower than target-operation coverage.

Removed whole-function-family tests are not a compatibility commitment. Rebuild
useful arithmetic, cleanup, crash, and reference cases against the common graph
as the missing support is implemented. Do not revive retired target forms merely
to preserve their historical test counts.

## Independent checking

The stage admits its inputs while it lowers: settlement bindings, the
installation join, the selected call index and each function's ABIs are bound in
[coordination](src/lowering/coordination.rs) as the program is produced, and a
raw fused-multiply-add plan is rejected there. The output is not re-validated in
the same compile; a program that violates this stage's contract is a bug in this
stage, for the corpus and the tests to find.

The target-to-selected
[independent audit](../03_target-operations-to-selected-instructions/src/legalization/scalar_graph_input.rs)
rejoins every graph to its source and optimized operations. The direct-native
corpus invokes it on the retained artifact, outside graph construction;
the default compile does not replay it. The audit checks source order, value
identity, exact ABI, proof, call, and cleanup obligations.
Root/roster custody alone never establishes body correctness.

Natural-ranked and unranked cycles use the same graph and native route, with
their respective verification obligations. The older unsigned-countdown native
custody is unsupported and rejects at admission; it cannot be stripped or
reinterpreted as ordinary proof. No dedicated countdown target or machine-code
carrier remains. This is an implementation limit, not a change to termination
or resource semantics.

## References, calls, and storage

Ordinary direct calls use one `TargetUnitOperation::Call` record for Unit,
scalar and structural results and any admitted scalar/structural argument mix.
Its `TargetCallResult` identifies the exact scalar home or structural result,
declaration, storage and returned custody. Bare references retain their original
referents without inventing a physical result home; aggregates retain their
exact storage. Only Unit has no result. The independent audit reconstructs the
result against pre-call custody; production retains the resulting origins for
later storage joins.
One [direct-call lowering](src/lowering/control_flow/calls.rs) produces
every such row: it resolves the callee contract and each operand's storage,
then commits moves and publishes the result. The result kind does not choose
argument transport. Published scalar operations additionally retain their
standalone ABI contract. The independent audit replays those rows through one
`direct_call` replay with a tail per result kind.
Source-specific semantic restrictions remain checked independently. Foreign
calls still have their own carriers; this is not a claim that the full call
inventory is normalized.

Operation dispatch has no whole-operation borrow preflight. Ordinary calls
select the existing descriptor metadata beneath each owned operand's exact
carrier prefix, retain it while lowering addresses, then commit its relocation.
They do not rebuild a type-wide leaf roster or recheck pairwise argument aliasing
and suspension. This preserves case namespaces without manufacturing metadata
for an inactive payload; source legality belongs to Psi. Ingress names possible
descriptor slots with `Case`/`Field` guards. Construction supplies only its
selected payload's descriptors, and cleanup disposes only resident metadata.
Those guards survive branch joins; they describe conditional storage, not a
grant of concurrent permissions for all alternatives. Transporting a descriptor
needs its referent identity, not a transitive pointee storage catalog. Actual
pointee projections still require declarations and concrete layout.
At a join, descriptor metadata follows the carrier's SSA storage definition.
Branch-local observations do not require another arm to recreate their homes;
all surviving carriers still have to agree on custody across actual edges.
Formation, release, edges and returns still have local native custody
restrictions. Their removal requires replacing the
storage relationship, not skipping the independent corpus audit.

[Structural signatures](src/lowering/structural_signature.rs) derive placement
from structural declarations and referent layout. Every borrowed access uses
`BorrowedReference`; only owned values receive shape-selected value placement.
A staged pointer is not a staged copy of its referent. See the
[reference contract](../../../../../wiki/spec/terminal-psi/structural_access.md).
Retained call arguments are reconciled against the source place, path, and
access, the callee declaration's access and type, their reconstructed shape,
the callee plan row placement, and caller-parameter roots with field-only
paths' root type and projected byte offset. The embedded call plan itself is
not authority: it must equal the plan independently derived from the callee's
declared signature, and each scalar argument must bind its source value,
ordinal, and the verified plan row's placement. Each structural argument's
retained source must also name the referent's canonical caller-side home: the
incoming parameter placement, a non-entry block parameter, or the exact
establishing operation — a staged destination, a sibling's home, or a copied
referent cannot substitute for the original storage.
Standalone receiving entrances — the graph's own scalar parameter rows and the
published `scalar_abi`/`mixed_structural_scalar_abi` a function exposes to
callers outside the plan — replay the same independently derived signature:
each scalar and result row must bind the declared value, type, and canonical
plan placement, and a published ABI is admitted only for the family its
producer derives. A coherent plan on an ineligible function, or either ABI
form beside the other, is a substituted entrance.

For a mutable fixed-byte-array view, the projected storage type is the array
while the callee receives a view descriptor. Validation reconstructs the exact
byte element, array extent and unit stride alongside the original root and
offset; neither descriptor type equality nor a producer-supplied length suffices.

Graph operations retain original invocation places, projections and widths.
Byte views retain their backing pointer, length, checked slice derivation, and
bounds obligations. Mutable stores address the original referent. Calls retain
the selected callee, ordered arguments, result home, exact ABI and contract.
Parallel transfers must preserve duplicate sources and cycles before replacing
destinations. Allocation owns the actual snapshots and frame locations.

A shared byte-view argument may observe bounded-owner storage without taking
ownership of its capacity or predicates. [Byte arguments](src/lowering/control_flow/byte_arguments.rs)
resolve the exact entry placement, state arrival or operation home and retain
its backing identity. Whole owners and inline owner fields present the same
live length word followed by bytes; the call-local descriptor contains the
backing address and current length, never capacity as length. Qualification
discharge remains with the verified source call: an unqualified read-only view
does not require removing the owner's qualifications or copying its predicate
roster to the formal. The receiving stages independently reconstruct geometry,
source identity, dominance and call placement. Concatenation is not an
eligibility condition for borrowing an established home.

Case observations resolve their root through the live storage inventory and
the canonical entry/state declaration. A descriptor carried into a state is
still observable; an unavailable declaration cannot revive it. Tag layout and
runtime selectors remain separate from that lookup, so observing a case through
a slice does not confuse its descriptor with the addressed element.

A shared view field holds a two-word descriptor (backing pointer, then extent)
in its record. `StructuralByteViewFieldStore` replaces that descriptor with an
established literal's pointer and length. A literal denotes immutable image
storage: selection materializes its backing pointer with
`ImageAddress`, a PC-relative address (`adrp`/`add` on AArch64,
`lea [rip]` on x86-64) whose field object construction binds to the literal's
octets in the image's read-only byte-literal pool, and keeps only the two-word
descriptor in the establishing frame. The stored view therefore outlives every
root, so lowering admits the store in any activation and through a `Referent`
crossing, exactly where Terminal admits it. A stored view field reads its
extent from the second word, and lends its own descriptor, addressed from the
root, to a shared view formal; a zero-filled shared view field is the empty
view.

Plain record construction uses the existing aggregate result home. Each field
retains its declaration identity and evaluated scalar operand or completed owned
child. Target and legalized receiving checks reconstruct the field roster, scalar
carriers, child types, range obligations, shape, and storage offsets. Selection
initializes complete storage before publishing its address; nested copies use exact
child extents and incoming owned values use their captured ABI storage. Calls borrowing
a local record retain the original home. Direct and hidden-pointer aggregate results
use the same calling-plan custody. Record block arrivals, including nested records,
retain the existing aggregate edge transport and their own destination storage;
no literal-only constructor or separate record storage graph is retained.

Case construction uses `EstablishCase` with the same scalar-or-structural field
operands as records. Lowering checks the selected sum case's exact field roster
and reuses record source and affine-consumption checks; nested completed children
retain their declared types and whole owned source places. The existing recursive
sum layout determines storage. Descriptor relocation keeps the selected case
prefix on payload fields and no case prefix on a mixed shape's common fields;
copying descriptor bytes never copies their referents. Static projection owns
pointer-slot geometry, including case-qualified paths, rather than a second
call-specific path grammar.

Shared calls can borrow a constructed plain record's existing home, including
one produced by an ordinary call or selected at a block join. Shared field loans
from joined records retain the exact block home and declaration-derived offset.
The borrow passes its address, not copied
field values. An affine owner remains affine while its shared loan has
unrestricted multiplicity; cleanup and transfer obligations stay with the owner.
Integer and Boolean field observations use the common graph's explicit field-read
operation. Receiving checks derive the field offset and scalar type from the
declaration, then selection reuses the primitive load kernel and independent
replay. Direct reads use the same path for established record locals and owned
block arrivals: the actual source place is retained rather than relabeled as an
incoming parameter. Availability and field identity are checked separately from
physical layout, and each load produces a fresh scalar before later operations.
Bounded integer leaves use the same load and exact signed/unsigned carrier.
Construction range obligations and the original bounded field declaration remain
in the input and receiving checks; equal physical layout cannot erase the range
or authorize an unproved store. Four-target field-read controls exercise bounded
carriers alongside wrong paths, fields, result types, homes and physical offsets.
By-value record entry parameters retain their owned value ABI. Selection captures
their register fragments into one activation-local input home
when field observations or borrowed calls need an address. Whole record returns
and nested copies then use that current home; entry fragments cannot replace
bytes changed by a completed mutable borrow. Inline stack inputs use their
existing incoming value storage. Indirect owned record inputs retain
the ABI-prepared value copy through its exact register- or stack-passed pointer.
They remain owned values, not borrows of the caller's original record. Field
observations, nested construction, and whole-parameter returns share that backing
across calls. Input and result placement are independently selected by the
complete call plan; a stack-passed value may return through hidden result storage.
Whole owned record and primitive-array arguments share
[owned argument lowering](src/lowering/control_flow/owned_arguments.rs).
It retains the current source backing and exact callee placement, including
indirect value copies; pointer-slot and payload-copy offsets remain distinct.
Source-produced integer getter controls cover direct and call-produced
locals, tail completion, and nested scalar arguments. Mutable local receiver
storage remains a separate dependency; shared
record-field receiver projections retain their original pointer and typed path.

Integer XOR also follows ordinary scalar definitions, legalized operands and
selected bitwise instructions. Its x86-64 and AArch64 realizations independently
check the opcode, operand registers and flag effects; an AND instruction cannot
stand in for XOR even when both share register constraints.

This stage has no dynamic dispatch lowering. The named-implementation selector
route is tracked by the dynamic dispatch tasks on [TASKS.md](../../../../../TASKS.md);
a selection is never permission to substitute a direct call or fabricate an
implementation.

## Acceptance

The crate's Rust tests cover
[placed container geometry](src/lowering/structural_layout.rs).
The corpus (`tests/omega/{pass,fail,run}`) exercises this stage end to end,
including the arithmetic and storage fixtures.
The corpus native leg (`python3 tools/corpus_gate.py --native`) builds source
programs through this stage and executes the `run` tier and `*_exit` fixtures on
the host; cross-emission is not host-execution evidence.

Borrowed storage, descriptor, boundary and cleanup coverage must exercise the
caller-visible result and invalid provenance/ownership controls. A target-only
fixture is not native closure. The [execution board](../../../../../TASKS_OPTIMIZER.md)
tracks remaining native operation and independent replay work.
