# Abstract operations to target operations

This stage derives target operations and call placement from abstract operations.
Start at [lib.rs](src/lib.rs), then [function routing](src/lowering/function/mod.rs).
The target representation owns the resulting control, values, storage, calls,
and boundary data; this stage does not assign registers or frame offsets.

## Scalar lowering has one route

Every ordinary scalar-result function enters
[control-flow lowering](src/lowering/control_flow.rs). It requires explicit source
blocks and retains definitions, calls, branches, and returns in
`TargetControlGraph`. Missing block structure rejects. There is no flat-input
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

The removed scalar-family tests are not a compatibility commitment. Rebuild
useful arithmetic, cleanup, crash, and reference cases against the common graph
as the missing support is implemented. Do not revive retired target forms merely
to preserve their historical test counts.

## Independent checking

[Validation](src/validation/mod.rs) rejoins the target, semantic entry, complete
function roster, structural signatures, and selected settlements. The remaining
family catalog covers non-scalar forms; it is not scalar graph validation.

Scalar graphs receive no invented family receipt: the target-to-selected
[receiving checker](../target-operations-to-selected-instructions/src/legalization/scalar_graph_input.rs)
independently rejoins the graph to its source and optimized operations before
legalized/selected graph construction. It rejects the retired scalar-return
carriers rather than converting them back into a graph. Source order, value
identity, exact ABI, proof, call, and cleanup obligations remain checked.
Root/roster custody alone never establishes body correctness.

The separately admitted ranked-countdown route retains its existing proof and
resource contract. Unit and structural-result lowering have separate current
entrypoints; removing scalar fallback does not claim to consolidate those too.

## References, calls, and storage

[Structural signatures](src/lowering/structural_signature.rs) derive placement
from structural declarations and referent layout. Every borrowed access uses
`BorrowedReference`; only owned values receive shape-selected value placement.
A staged pointer is not a staged copy of its referent. See the
[reference contract](../../../../wiki/spec/terminal-psi/structural_access.md).

Graph operations retain original invocation places, projections and widths.
Byte views retain their backing pointer, length, checked slice derivation, and
bounds obligations. Mutable stores address the original referent. Calls retain
the selected callee, ordered arguments, result home, exact ABI and contract.
Parallel transfers must preserve duplicate sources and cycles before replacing
destinations. Allocation owns the actual snapshots and frame locations.

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
