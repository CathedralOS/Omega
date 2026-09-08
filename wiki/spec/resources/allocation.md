# Allocation strategies

Allocation strategies are ordinary checked packages, not compiler-owned Arena,
bump, slab, pool, buddy, or heap primitives. Their substrate is a qualified
linear `Extent`, exact separated split/merge and content conservation, checked
layout/access/cleanup, and boundary services for acquiring backing.

The caller supplies backing and its established properties. An allocator does
not reclassify it as RAM, MMIO, GPU, persistent, or other storage. Splitting and
accounting do not bypass the backing's establishment, access, transfer, or
cleanup rules. A generic allocation does not promise an ordinary `&mut T`;
non-RAM backing may require a [placed view](placed_access.md).

Using already-held storage need not reach a provider. Acquiring fresh backing
through a boundary contributes that service's reach and its evidence. A strategy
may offer a fallible request preserving unchanged state on rejection and an
infallible request whose caller proves the exact aligned capacity requirement.
Neither route grants storage authority from a number alone.

## Monotonic allocation and return

The bump-allocation customer distinguishes:

| State | Meaning |
| --- | --- |
| Allocatable tail | Concrete backing not yet issued by the monotonic cursor. |
| Live extent | Exact storage owned by an allocation value. |
| Retired extent | Cleaned storage returned to the strategy, unavailable until reset. |

Allocation temporarily borrows the strategy exclusively, splits an aligned
subextent, establishes the value, and returns its owned claim. That borrow ends
at return, allowing multiple allocations to coexist without new shared-mutation
or borrow-checking rules.

Release performs ordinary value cleanup and transfers the exact extent back as
retired content. It does not rewind the cursor. A release operation that does not
transfer the claim back cannot restore the strategy's authority or capacity.
Reset requires recomposition of the tail and every retired extent into the
original backing, proving no live allocation remains. Finishing likewise returns
that backing. Bulk reclamation never substitutes for debt-bearing value cleanup.

[Content custody](content_custody.md) owns the source-visible conservation
contract. A counted byte residual summarizes capacity; the tail extent supplies
placement. These are distinct premises.

## Reuse and fragmentation

Value cleanup, extent return, and immediate capacity reuse are separate
guarantees. A container requiring prompt reuse must ask for it rather than impose
it on every strategy. A container can be safe over monotonic allocation while
retaining old buffers until reset.

Total free bytes do not prove a sufficiently large contiguous region exists.
Fragmented allocation remains fallible unless exact placement or reservation
evidence establishes the request. A common interface must not assume bump
semantics merely because bump allocation is the first customer.

The [bump allocator task](../../../TASKS.md#p5---cathedral-over-general-omega-primitives)
owns implementation and its coexisting-allocation, cleanup, failed-live-reset,
recomposition, and backing-return controls. The contract does not claim that
the complete source substrate or package implementation already works.
