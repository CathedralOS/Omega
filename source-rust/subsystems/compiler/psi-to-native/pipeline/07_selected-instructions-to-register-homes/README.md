# Register allocation

`stage_register_allocation` assigns physical homes and admits the completed
allocation. It publishes an `AllocatedBody` in
[`register-homes/published`](../../representations/register-homes/published),
which contains the current selected program, homes, analyses, preservation and
publication facts. Its constructor requires the allocation issuer; consumers
cannot manufacture the product from bare plans.

The plans are shared through `Arc`; publishing does not copy their tables.
`AllocationResult` keeps optional producer replay evidence separate from that
product. The native driver retains replay evidence only for requested validators.
Machine lowering reads the product without linking this transformation crate.

Start at [`src/allocation/register_allocation.rs`](src/allocation/register_allocation.rs).
The algorithms and their private evidence remain under `src/allocation/`.
