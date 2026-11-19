# Allocation strategies

Allocation strategies are ordinary checked packages, not compiler-owned Arena,
bump, slab, pool, buddy, or heap primitives. Their substrate is a qualified
linear `Extent`, exact separated split/merge and content conservation, checked
layout/access/cleanup, and boundary services for acquiring backing.

The caller supplies backing and its established properties. An allocator does
not reclassify it as RAM, MMIO, GPU, persistent, or other storage. Splitting and
accounting do not bypass the backing's establishment, access, transfer, or
cleanup rules. A generic allocation does not promise an ordinary `&mut T`;
non-RAM backing may require a [access view](access.md).

Using already-held storage need not reach a provider. Acquiring fresh backing
through a boundary contributes the requirement's explicit reach and its evidence.
The boundary's name alone contributes no reach. A strategy
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

## Dense collections over allocated backing

A library `Vec<T>` uses a [live typed run](typed_runs.md) for its established
prefix and adjacent granted vacant backing for spare capacity. Its library
implementation composes the compiler-realized cell operations with ordinary
vacant-extent transformations:

| Operation | Checked steps |
| --- | --- |
| Push within available capacity | Use `split_vacant_extent` to take one cell's footprint from the front of spare backing; call `Run::initialize_cell`; merge the successful singleton onto the live run with `Run::merge`. |
| Rejected initialization | Recover the returned vacant cell and original input value; use `merge_vacant_extents` to restore the spare tail before returning the value. The existing live run remains unchanged. |
| Pop from a nonempty vector | Use `Run::split` at count minus one; extract the last singleton with `Run::extract_cell`; merge its vacated backing onto the front of the spare tail with `merge_vacant_extents`. |

The vector's invariant relates both fields to the same allocated region. Taking
and returning a cell at their shared boundary supplies the adjacency facts for
these calls. Each successful primitive's contract must carry those facts to the
next call. A rejected initialization restores the original spare geometry and
custody; a failure after partial establishment must account for the actual live
outputs before claiming vacancy. The completed operation restores the vector's
invariant on every returning path.

Growth acquires compatible backing and relocates the live run before returning
the old backing under its allocation contract. Rejected acquisition or relocation
preserves the original live run and accounts for any newly acquired backing.
Allocation, capacity checks, growth and recovery remain ordinary library code.
These operations conserve existing allocator authority.

This representation has no per-element occupancy tag. Linear values, contained
loans and cleanup obligations remain owned by the live run. A zero-sized element
still contributes one logical cell and its obligations even though it consumes
no payload bytes. Allocation failure preserves the existing vector and accounts
for any attempted input. Growth policy and fallible versus pre-proved capacity
remain ordinary library choices.

Acceptance requires one integrated Vec implementation covering linear elements,
zero-sized elements with cleanup obligations, contained loans, fallible growth,
and rejected initialization after splitting the spare tail. It must preserve
element order and exactly-once disposition, restore the spare tail on rejection,
and reject duplicate ownership or invalidated loans. Split and merge obey the
[constant runtime cost contract](typed_runs.md#cost-and-compiler-boundary)
through both execution routes, including during element-by-element disposal.

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
recomposition, and backing-return controls.
