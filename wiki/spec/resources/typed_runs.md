# Dense owned runs

Named decision: `typed-run-custody`, ratified 2026-11-02.
Cell-transfer surface: `typed-run-cell-transfer-surface`, ratified 2026-11-07.

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

Initialize, extract, split, merge and relocate have compiler-realized custody
semantics. The exact core declarations supply the cell-transfer rules; callers
prove their ordinary contracts. Byte conservation, count equations and per-child
geometry remain necessary but do not alone prove custody preservation. Zero-sized
splits partition logical positions and counts, even when all byte boundaries
coincide. Each actual cell keeps its obligations. Live loans prevent consuming
transformations that would invalidate them.

Rejection/failure paths account for every input and every established output;
failed initialization cannot lose the input value or its backing. A partial
operation must expose its actual retained custody rather than report vacancy.
Crash and abandonment follow the existing survivor rules, not invented rollback.

Relocation is a semantic move, not an unrestricted byte copy. For compatible
ordinary RAM with movable elements and a valid disjoint destination it may lower
to a bulk memory copy, with no element cleanup during the move. Other placements
need permitted element operations; a policy does not promise that movement is
available. Pinned elements cannot gain mobility by entering a run.

## Split and merge contracts

`Run::split<P, T>` and `Run::merge<P, T>` are public, bodyless compiler-supplied
core machines admitted through the [sealed catalog](../language/machines.md#supply).
Their exact declaration identities and signatures own the trusted rules below.
Matching a helper's name or adding it to `established by` grants no primitive
semantics. `Live<P, T>` includes both declarations among its authorized
establishment routes, alongside its existing empty, initialization and relocation
routes.

| Primitive | Caller obligations | Custody result |
| --- | --- | --- |
| `Run::split<P, T>(run, cut)` | Own a live run and prove `cut <= run.count`, with the backing partition and outstanding loans valid. | Consume `run`; return `RunParts<P, T>` with live `left` and `right` fields owning exactly its prefix and suffix. Their counts are `cut` and the input count minus `cut`. |
| `Run::merge<P, T>(left, right)` | Own both live runs; prove compatible placement, backing and carry obligations, valid backing adjacency, and that the combined count fits `u64`. | Consume both inputs; return one live run containing the actual left cells followed by the actual right cells, with their combined count and backing. |

Both operations are total when their preconditions hold. Bounds, adjacency and
compatibility are `requires` obligations; a caller with runtime inputs can branch
to establish them. Neither operation has a rejection outcome, a `Role` parameter,
a resource-profile argument or a provider lookup. They partition or join existing
ownership without accessing payloads, allocating storage or relocating elements.
They need no supply-selecting wrapper.

For example, splitting and rejoining transfers the original ownership throughout:

```omega
let parts: RunParts<P, T> = Run::split<P, T>(run, cut);
let whole: Run<P, T> in Live<P, T> =
    Run::merge<P, T>(parts.left, parts.right);
```

At each call, the consumed inputs become unavailable. The compiler tracks the
resulting cell ranges and retained obligations symbolically through ordinary
ownership flow, call contracts and joins. It does not enumerate the runtime
count or maintain a runtime registry of runs.

Cuts at zero and at the full count produce an authorized empty partition and
the unchanged occupied subsequence. Empty means zero cells. A zero-byte run with
a positive count remains occupied: merging it preserves argument order and every
cell's custody, and releasing its backing still requires disposition of those
cells. Equal addresses never substitute for this ordered transfer.

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

A shared view `&[f32 in Km]` may be read as `&[f32]` under the existing recast
rules. The removal is written, not implied; its spelling for a domain on the
element type is [left open](../../drafts/designs/named_domain_casts.md#left-open).
A mutable view may omit only element domains the lender could add again under
[mutable views](../language/domains.md#mutable-views), and must preserve
or restore the original domain predicates before its loan ends. Nominal units with no value
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

Each split or merge performs constant runtime work independent of element count:
descriptor arithmetic only, with no per-element access or custody walk in native
emission or the interpreter. This also applies to each split used by a consuming
traversal. Required cleanup of `n` elements may take `n` cleanup calls; splitting
off each next cell must not add a scan of the remaining run.

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
