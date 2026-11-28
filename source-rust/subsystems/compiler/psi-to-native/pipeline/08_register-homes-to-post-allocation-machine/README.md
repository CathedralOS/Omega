# Lower allocated homes to a machine plan

`stage_optimized_post_allocation_machine_plan` consumes the allocator's sealed
[`AllocatedBody`](../../representations/register-homes/published) and produces
the post-allocation machine plan. It reads assigned homes, liveness ranges,
legality, preservation and publication facts from that representation.

This stage does not depend on the allocator implementation and cannot acquire
the capability that issues its input. Optional replay evidence stays with the
native driver and published-object verification.

Start at [`src/machine/mod.rs`](src/machine/mod.rs); `plan` constructs the machine
operations and the selected optimization phase transforms that plan.
