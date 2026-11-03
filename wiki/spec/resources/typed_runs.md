# Dense owned runs

Named decision: `typed-run-custody`, ratified 2026-11-02.

A growable collection must own a runtime number of values without inserting a
tag beside every element. `Run<P, T>` supplies that storage contract: contiguous
cells containing established values, with their ownership obligations retained.
`Vec<T>` remains ordinary library code over a live run and adjacent vacant spare
backing. Allocation strategy and growth policy are not compiler primitives.

## Carrier and establishment

`Run<P, T>` is a linear core carrier with compiler-understood custody semantics,
not a keyword or a special case for Vec. Its descriptor retains granted backing
and an explicit `u64` element count. Its protected `Live<P, T>` qualification means
each occupied cell holds an established `T` and the run owns that value's complete
obligations, including non-runtime fields and contained loans.

As in [resident custody](access.md), `P` identifies the domain access policy;
the independently selected validated layout and its correspondence remain in
occurrence evidence. A policy contains no layout. The cell footprint comes from
that established placement, not an unconditional `size_of<T>()` calculation.
Ordinary placement uses the native array stride for `T`. Checked mathematical
arithmetic establishes that count times footprint equals the run's occupied byte
length, together with alignment and backing compatibility. Spare capacity is not
part of that occupied range.

Zero-footprint cells still have an explicit count and real obligations. Their
positions are logical indexes even though their byte boundaries coincide. Empty
storage or increasing a count cannot establish additional values. An empty run
owns no cells; it can be formed from an authorized empty partition without
creating backing authority or content.

`Live` declares its authorized routes through ordinary domain access policy.
It cannot be freely established, cast away, or inferred from valid-looking bytes
and a count. `Granted` supplies backing authority; the allocation contract, not
`Granted` itself, determines who must receive returned storage. `Vacant` states
absence of live content, not zero initialization.

## Retained custody and transformations

A run retains opaque custody of each occupied cell. Its public contract exposes
no individual issuance identity. Extraction transfers the actual selected value
and its obligations; it never issues replacement authority from type and count.
Different cells may hold different resources despite having the same type.
Hiding identity from callers is not permission to discard it from custody.

The run retains the restrictive combination of its cells' carry permissions and
one compatible carry anchor; insertion of a conflicting anchor rejects. Contained
loans retain their lenders and lifetimes conservatively. Forgetting permission
never forgets the associated obligation. Calls and control-flow joins preserve
this custody summary without enumerating the runtime count.

A run does not project its cells' content as an aggregate content sum. For example,
a run of granted extents holds those claims internally until extraction or an
authorized disposition. Insertion is not a custody exit. Hiding the aggregate
projection neither permits duplicate extraction nor lets backing become vacant
while live elements remain.

The checked transformations are:

| Operation | Required transformation |
| --- | --- |
| Initialize one cell | Transfer an owned, established `T` into compatible exclusive vacant backing and produce a singleton live run. |
| Extract one cell | Consume a singleton live run, returning its actual `T` and the vacated backing. |
| Split | Partition the ordered cells at a proved index, preserving the two subsequences and their backing correspondence. |
| Merge | Concatenate compatible ordered runs with adjacent backing, retaining all custody. |
| Relocate | Transfer the cells and their obligations into compatible vacant backing, leaving the source vacant. |

Initialize, extract and relocate have compiler-realized custody semantics.
Split and merge are ordinary library machines whose contracts and bodies prove
the typed-cell partition/concatenation. Byte conservation, count equations and
per-child geometry are necessary but do not alone prove custody preservation.
Zero-sized splits partition logical positions and counts, not just byte ranges.
No operation duplicates cells or silently swaps the association between a cell
and its obligations. Live loans prevent consuming transformations that would
invalidate them.

Rejection/failure paths account for every input and every established output;
failed initialization cannot lose the input value or its backing. A partial
operation must expose its actual retained custody rather than report vacancy.
Crash and abandonment follow the existing survivor rules, not invented rollback.

Relocation is a semantic move, not an unrestricted byte copy. For compatible
ordinary RAM with movable elements and a valid disjoint destination it may lower
to a bulk memory copy, with no element cleanup during the move. Other placements
need permitted element operations; a policy does not promise that movement is
available. Pinned elements cannot gain mobility by entering a run.

## Views, mutation and release

Ordinary native placement over suitable Stable RAM admits `&[T]` and `&mut [T]`
over the live cells, with the same bounds, authority and loan obligations as
array views. Encoded, device or otherwise incompatible placement instead uses
its permitted per-cell access operations; it cannot fabricate a native slice.

Element mutation follows ordinary loan and restoration rules. Replacement and
swap transfer actual values and their obligations. Taking a value through an
exclusive view opens the existing restoration window; the owner cannot escape
with an unaccounted hole. Dense `remove`/`swap_remove` repair the live prefix and
reduce its count. Persistent holes require an explicit representation such as
`Optional<T>`, not hidden run liveness flags.

Predicate-only qualification erasure permits reading `&[f32 in Km]` as `&[f32]`
under the existing recast rules. A mutable view must preserve or restore the
original domain predicates before its loan ends. Nominal units with no value
predicate need no extra value test; arbitrary predicate domains do. Access,
carry and authority restrictions cannot be cast away, even inside a window.

Release consumes or legitimately disposes every live value before returning
vacant backing under the allocation contract. A length assignment is not a
disposition. A library may supply affine cleanup only when its complete element
and backing disposition meets the existing drop contract; a linear element is
not made automatically droppable by putting it in a collection.

## Cost and compiler boundary

The payload is dense: no per-element tag, runtime custody table, occupancy bitmap
or hidden ownership update is required. Custody evidence is erased. Descriptor
storage does not grow with the element count. Actual payload data, counters,
allocator metadata and cleanup effects retain their ordinary costs.

An authored disposal traversal may consume one cell and retain a smaller run at
each iteration. The compiler checks that transition and its loop invariant
symbolically, not once for every possible runtime element. A cursor or remaining
count is ordinary runtime state. Swapping needs only the data movement; popping
transfers the element and adjusts the descriptor; cleanup visits live elements
when their actual cleanup requires it. This does not promise constant compiler
cost for arbitrary proofs, nor remove the work performed by destructors.

This is general dense-storage ownership support in checking, Terminal Psi and
native realization, not a spelling recognized only around Vec calls. One contract
covers trivial scalars, qualified values, sums, non-copy values and zero-sized
obligations; trivial custody operations may optimize away.

`Resident<P, T>` remains the single-value contract. Checked conversion to and from
a one-cell run must preserve field-level partial moves, non-runtime custody,
loans, layout/footprint and any excess backing. Folding the two carriers into one
implementation is valid only after those semantics agree; it cannot silently
change existing resident placement or retirement contracts.
