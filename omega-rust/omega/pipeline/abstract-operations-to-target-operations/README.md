# Abstract operations to target operations

This stage derives target operations and call placement from abstract operations.
Start at [lib.rs](src/lib.rs), then [function routing](src/lowering/function/mod.rs).
The target representation owns the resulting control, values, storage, calls,
and boundary data; this stage does not assign registers or frame offsets.

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

[Validation](src/validation/mod.rs) rejoins the target, semantic entry, complete
function roster, structural signatures, and selected settlements. This receipt
does not establish body correctness; there is no whole-function family catalog.

The target-to-selected
[receiving checker](../target-operations-to-selected-instructions/src/legalization/scalar_graph_input.rs)
independently rejoins every graph to its source and optimized operations before
legalized/selected graph construction. Source order, value
identity, exact ABI, proof, call, and cleanup obligations remain checked.
Root/roster custody alone never establishes body correctness.

Natural-ranked and unranked cycles use the same graph and native route, with
their respective verification obligations. The older unsigned-countdown native
custody is unsupported and rejects at admission; it cannot be stripped or
reinterpreted as ordinary proof. No dedicated countdown target or machine-code
carrier remains. This is an implementation limit, not a change to termination
or resource semantics.

## References, calls, and storage

[Structural signatures](src/lowering/structural_signature.rs) derive placement
from structural declarations and referent layout. Every borrowed access uses
`BorrowedReference`; only owned values receive shape-selected value placement.
A staged pointer is not a staged copy of its referent. See the
[reference contract](../../../../wiki/spec/terminal-psi/structural_access.md).
Retained call arguments are reconciled against the source place, path, and
access, the callee declaration's access and type, their reconstructed shape,
the callee plan row placement, and caller-parameter roots with field-only
paths' root type and projected byte offset.

Graph operations retain original invocation places, projections and widths.
Byte views retain their backing pointer, length, checked slice derivation, and
bounds obligations. Mutable stores address the original referent. Calls retain
the selected callee, ordered arguments, result home, exact ABI and contract.
Parallel transfers must preserve duplicate sources and cycles before replacing
destinations. Allocation owns the actual snapshots and frame locations.

Plain record construction uses the existing aggregate result home. Each field
retains its declaration identity and evaluated scalar operand or completed owned
child. Target and legalized receiving checks reconstruct the field roster, scalar
carriers, child types, range obligations, shape, and storage offsets. Selection
initializes complete storage before publishing its address; nested copies use exact
child extents and incoming owned values use their captured ABI storage. Calls borrowing
a local record retain the original home. Direct and hidden-pointer aggregate results
use the same calling-plan custody. Scalar-field record block arrivals retain the
existing aggregate edge transport. Nested record block arrivals remain unsupported;
no literal-only constructor or separate record storage graph is retained.

Shared calls can borrow a constructed plain record's existing home, including
one produced by an ordinary call. The borrow passes its address, not copied
field values. An affine owner remains affine while its shared loan has
unrestricted multiplicity; cleanup and transfer obligations stay with the owner.
Integer and Boolean field observations use the common graph's explicit field-read
operation. Receiving checks derive the field offset and scalar type from the
declaration, then selection reuses the primitive load kernel and independent
replay. Direct reads use the same path for established record locals and owned
block arrivals: the actual source place is retained rather than relabeled as an
incoming parameter. Availability and field identity are checked separately from
physical layout, and each load produces a fresh scalar before later operations.
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
General indirect owned argument forwarding remains a separate transport limit.
Source-produced integer getter controls cover direct and call-produced
locals, tail completion, and nested scalar arguments. Mutable local receiver
storage remains a separate dependency; shared
record-field receiver projections retain their original pointer and typed path.

Integer XOR also follows ordinary scalar definitions, legalized operands and
selected bitwise instructions. Its x86-64 and AArch64 realizations independently
check the opcode, operand registers and flag effects; an AND instruction cannot
stand in for XOR even when both share register constraints.

Dynamic descriptor operations may be representable here without native indirect
call support. Their source/table/slot identities are not permission to substitute
a direct call or fabricate an implementation. Object, image and installation
replay must bind the actual call, table, relocation, stack and code intervals.
See the descriptor recovery task on [TASKS.md](../../../../TASKS.md).

## Acceptance

[Scalar value regressions](src/tests/scalar/shared_values.rs) check bounded
per-operation storage and once-only definitions.
[Source-produced native controls](../../../../tests/native-differential/tests/scalar_control_cycles.rs)
exercise common scalar control, call-result reuse, independent corruption checks,
and four-target publication. Runtime checks require a supported matching host;
cross-emission is not host-execution evidence.

Borrowed storage, descriptor, boundary and cleanup coverage must exercise the
caller-visible result and invalid provenance/ownership controls. A target-only
fixture is not native closure. The [execution board](../../../../TASKS_OPTIMIZER.md)
tracks remaining native operation and independent replay work.
