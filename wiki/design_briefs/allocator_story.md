# Design Brief: Arena Allocation Story

Current direction; implementation remains staged in `TASKS.md`. The allocator
surface uses an explicit bounded Arena capability and dependent contracts.
Allocator boundary reach is a service member. Quantitative row entries wait for
the resource-algebra brief.

This brief supersedes the old allocator use of `Region`. A region in ordinary
memory prose means a span; Omega's precise authority over a concrete span is an
`Extent`. The allocation capability is an `Arena`.

## Vocabulary and relationships

| Concept | Meaning |
|---|---|
| `Extent` | authority over one concrete address-space range |
| `Arena` | bounded, lifetime-scoped allocation domain, backed by an Extent or provider |
| `Allocation<T>` | arena-bound owned storage with layout and establishment state for `T` |
| `Allocator` | deferred general interface for strategies weaker/different than Arena semantics |
| budget/reservation | quantitative capacity fact or token, not the storage object |
| pool | fixed-size/fixed-class allocation strategy, normally a package over an Arena or provider |

These are core/library abstractions over existing ownership, lifetime, domain,
and provider machinery; this settle adds no grammar. Capitalized `Arena` names
the capability. Lowercase "arena" may describe an implementation strategy only,
never a second semantic object.

The dependency chain is:

```text
Extent or admitted provider backing
    -> Arena borrow/capability
        -> Allocation<T>
            -> slices and placed/typed views
```

Each child is lifetime-bounded by its parent. Arena reset or reclamation is
illegal while any Allocation remains live; an Allocation cannot outlive the
Arena from which it was issued.

## Multiplicity follows representation

Multiplicity is structurally derived rather than chosen as allocator policy.

- `Arena::over(&mut backing)` borrow-carries an Extent and is affine. Dropping
  it ends allocation permission after all Allocations have ended.
- A form that consumes and stores an owned linear Extent necessarily derives
  linearity. It is a distinct owned-backing wrapper/lease and must return or
  release the backing explicitly.
- `Allocation<T>` derives its multiplicity from its fields and `T`. Storing a
  linear value makes the allocation/container debt-bearing; consuming it must
  move or terminally consume every obligation before storage is reclaimed.

Bulk reclamation never substitutes for element consumption. The Allocation's
borrow blocks reset while it is live, so arenas may semantically store affine or
linear values without laundering debt. An initial implementation may reject
debt-bearing `T` until generic structural-linearity enforcement lands; that is a
dated engineering fence in `TASKS.md`, not language doctrine.

## Storage lifecycle and ZII

Allocation grants storage, not a live `T`:

```text
reserved storage -> initialized/established storage -> live T -> reclaimed
```

Reads gate until establishment. Zero-expressibility does not assert zero
contents: recycled storage may contain arbitrary prior bytes. A provider may
offer actually-zeroed storage, but immediate establishment is legal only when
zero honestly establishes `T`; otherwise construction must establish the
required value/domain facts first.

Establishment and non-disclosure protect different parties:

- establishment prevents a new checked reader from consuming invalid bytes;
- provider-established non-disclosure prevents a prior principal's bytes from
  leaking when storage becomes visible across a trust boundary.

Non-disclosure must hold before recipient visibility and cover the entire
recipient-visible range, including padding and page slack. Scrubbing, complete
authorized overwrite, narrowed mapping, or an equivalent admitted policy may
establish it. Recipient-side establishment never discharges previous-owner
confidentiality.

## Current state

- `Vec<T>` is browsable surface (`omega/language/core/vec.omg`) with no runtime storage
  lowering. `String` remains statically fixed-capacity.
- The provider registry reserves the allocation-service seam needed for host or
  Cathedral backing.
- Lifetimes make arena-borrowing containers expressible.
- Extent, Arena, and Allocation are separate relationships. An Arena may borrow
  an Extent; it never manufactures fresh range authority.

## Recommendations

1. **No ambient heap.** Allocation requires an explicit Arena or later
   Allocator capability.
2. **Arena v1.** Ship the stronger bounded, bulk-lifetime abstraction before a
   general pluggable Allocator interface.
3. **Vec ladder.** Keep fixed-capacity containers as the permanent floor; then
   add fixed-capacity `Vec<'a, T>` backed by an Arena Allocation, with no growth;
   add pluggable growth only if a real customer requires it.
4. **Proof-obligated capacity.** Proven sites allocate infallibly under
   `requested <= remaining`; genuinely dynamic sites use an explicit fallible
   reserve/allocation outcome. No silent OOM trap.
5. **Cleanup.** Containers discharge their elements when consumed/dropped under
   ordinary multiplicity rules. Arena bulk reclamation begins only after every
   Allocation ends.
6. **Service reach.** Only reaching the allocation provider contributes its
   boundary-service member; ordinary reads/writes of already-owned allocations
   do not.

## Staging

1. Heap-free fixed-capacity containers and Arena contract surface.
2. Borrow-backed Arena plus fixed-capacity `Vec<'a, T>`, establishment gates,
   element cleanup, and bulk reclamation.
3. Debt-bearing generic elements after structural multiplicity through generics
   is enforced.
4. General `Allocator` interface, growth, and movable storage only when demanded.

## Touches

Runtime allocation ABI and descriptors, lifetimes, structural multiplicity,
drop/consumption analysis, effects, instruction selection, length/capacity
proofs, Extent-backed OS allocation, and cross-principal sanitization.
