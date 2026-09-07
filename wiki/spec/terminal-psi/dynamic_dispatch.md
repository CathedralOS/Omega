# Dynamic dispatch

Dynamic calls retain the descriptor or parameter slot that selects their
realization. They do not replace that selection with a static callee or a copy
of its source expression.

## Complete application identity

Each selection retains the complete canonical `ClosedConformanceApplication`:
telescope, ordered requirement-row map, callable registry, report coordinate,
and strong commitment. Selecting one row does not permit dropping or reordering
unselected rows. Verification rejoins the application to the exact selected row,
callable, carrier type, access, result, and service reach.

A rebound descriptor retains initializer and latest selections and their
version relationship. Rebinding to a different named conformance preserves the
exact carrier, dynamic-trait interface, borrow access, telescope, and ordered
requirement roster. The two applications have independent commitments. Only
the latest supplies the executable table; an overwritten initializer remains
identified without inventing an unused table.

Call composition, reachability, verification, interpretation, optimization
validation, and logical-work derivation resolve the same selection. Orphaned,
duplicate, reordered, retargeted, or static-call-substituted records reject.
Knowing the latest selection is not permission to devirtualize its call.

## Call and forwarding roles

| Role | Unit call representation |
| --- | --- |
| Direct local selection | `CallUnit` with its exact direct-dispatch row. |
| Rebound descriptor | `CallDynamicUnit` with descriptor ordinal and exact selected requirement row. |
| Forwarded descriptor parameter | `CallDynamicParameterUnit` with the helper's parameter slot and requirement row. |

Dynamic scalar calls retain the corresponding scalar result type and identity.
Unit calls retain obligations and crash continuations but have `OperationResult::Unit`:
no invented value ID or result home. Scalar results use an exactly typed durable
home through the ordinary caller and helper; Boolean results do not become
integer-shaped placeholders.

Owner-local selections, rebound descriptors, and inbound parameters are distinct
argument sources. Forwarding preserves that role rather than relabeling all
sources as descriptors. An outer descriptor call and its helper's parameter-slot
call remain two independently identified operations.

## Native tables and replay

Native realization uses the canonical `{ instance, table }` carrier. Descriptor
and scalar-result storage do not overlap. Borrowed instances obey
[reference identity](structural_access.md); a descriptor does not authorize
copying a borrowed referent.

Immutable tables may deduplicate identical complete applications by strong
identity. Emit one slot per canonical row. A row with an executable callable
binds its exact function symbol; a semantic row without a callable retains a
zero trap slot. Do not compact it away or invent a function.

Independent object and final-image replay checks descriptor bytes, table-address
relocations, selected-slot load/call, function-slot relocations, and final data.
Retained evidence binds complete table/slot projection and each call's
application, sources, selected slot, realization, and code interval. Installed
replay preserves the final text/data layout and section gap, exact relocation
addresses, and pre-relocation and final bytes. This immutable-table contract
does not imply mutable data or BSS support.

## Mutation-bearing realizations

A realization's ordered stores are distinct from its return expression. Each
store retains the exact receiver/path, primitive field, typed value, and source
order; subsequent reads retain their own identities. Lowering and replay
reconstruct all field offsets from declarations. A subloan of an unrestricted
mutable root remains unrestricted, not an invented linear entry claim.

The [structural store contract](structural_access.md#store-vocabulary) governs
access and write visibility. Store emission followed by a successful dynamic
call is insufficient unless the caller's original referent reflects the writes.

Current source and native subsets are described beside
[Terminal-to-abstract lowering](../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md#dynamic-dispatch).
They are implementation limits, not additional source semantics.
