# Design Brief: Allocation Substrate and Strategy Packages

Status: semantic direction settled 2026-07-31; implementation remains staged
in `TASKS.md`.

Omega does not make an Arena, bump allocator, slab, pool, buddy allocator, or
general heap a language primitive. Core supplies the reusable substrate:

- `Extent`, a linear claim over concrete storage carrying placement and access
  qualifications;
- exact separated split/merge and content conservation;
- layout, establishment, access, cleanup, and non-disclosure rules for values
  placed in an extent; and
- boundary services from which a package may obtain new storage.

Allocation strategies are ordinary checked packages over that substrate. A
bump-allocation implementation is the acceptance canary, not a blessed core
abstraction. Its public name is a package choice.

## Backing and access

The caller chooses backing by supplying an appropriately qualified `Extent`.
The strategy inherits its placement and access properties; it does not decide
that storage is RAM, GPU memory, MMIO, persistent memory, or another class.

Splitting and accounting are independent of placement. Establishing, reading,
writing, transferring, and cleaning a `T` still require the selected backing's
ordinary layout and access evidence. Therefore a generic allocation result does
not promise `&mut T`: RAM-backed storage may lend an ordinary mutable view,
while GPU or other placed storage exposes the corresponding placed-view API.

## Bump-allocation canary

The canary consumes one backing extent and represents three distinct states:

```text
allocatable tail   storage not yet issued by the monotonic cursor
live extent        owned by an allocation value
retired extent     returned after cleanup, but unavailable until reset
```

Allocation takes exclusive access to the strategy only for the call. It splits
an aligned subextent from the tail, establishes `T`, and returns an owned linear
claim. The borrow ends when the call returns, so several allocations coexist
without shared-borrow mutation, interior mutability, lease counters, or a new
borrow-checker rule.

Release runs `T`'s ordinary cleanup and transfers the exact live extent back to
the strategy as retired content. It does not rewind the cursor or restore
allocatable capacity. A standalone `allocation.release()` is insufficient
unless it returns authority that is subsequently transferred back; the
operation must conserve the claim explicitly.

Reset is legal only after the allocatable tail and every retired extent
recompose to the original backing, proving that no live allocation remains. It
then rewinds the cursor and restores the tail. Finishing the strategy similarly
returns the original backing extent. Bulk reclamation never substitutes for
cleanup of debt-bearing elements.

`CountedQuantity<Bytes>` may summarize residual magnitude for a capacity proof,
but a scalar byte count does not identify where storage lies. The residual tail
extent supplies placement for the bump canary. The source spelling for the
n-to-m split, custody exit, and recomposition theorem is settled in
[`authority_values_and_boundary_evidence.md`](authority_values_and_boundary_evidence.md);
implementation remains staged in `TASKS.md`.

## Failure and provisioning

A package may expose both operations:

- a fallible request that preserves and returns the unchanged state on
  rejection; and
- an infallible request whose caller proves adequate aligned tail capacity.

Provisioning storage and reaching a storage provider remain separate. Using an
already-owned extent need not reach a provider. Obtaining fresh backing through
a boundary service contributes that service's reach and its admitted or derived
evidence.

## Retirement is not reuse

Allocator contracts must keep three properties separate:

1. retire an allocation and clean its value;
2. return its extent authority to the allocator; and
3. make that capacity immediately reusable.

A bump strategy supports the first two and delays the third until reset. A
container can therefore be correct over such a strategy while retaining old
buffers and wasting capacity. Containers that require prompt reuse should ask
for that stronger capability explicitly rather than make it part of every
allocation interface.

General fragmented allocation is a later container-driven problem. Total free
bytes do not prove that one sufficiently large contiguous extent exists, so
such an allocator remains fallible or supplies exact placement/reservation
evidence. Do not shape a common allocator interface around the bump canary
before that customer is implemented.

## Implementation staging

1. Implement the settled source-visible content-conservation contract.
2. Implement the bump-allocation canary as ordinary package code over `Extent`,
   placement, and conservation.
3. Require canaries for coexisting allocations, failed reset with a live claim,
   release into retired content, successful reset/recomposition, backing return,
   and RAM versus non-RAM access.
4. Treat any failure to express the canary as evidence of a substrate gap, not
   as permission to add an Arena primitive.
5. Design reusable fragmented allocation when the container/backend customer
   is ready.

The existing core `Arena` boundary trait is an early service seam, not evidence
that bump semantics belong in core. Its eventual name and interface should be
judged with the general allocator customer; no current owner question depends
on it.
