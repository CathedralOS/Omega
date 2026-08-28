# Omega product source

This root owns the Omega-written target-lowering, optimization, and
artifact-emission half of the production compiler; implementation remains open.
Its source closure deliberately uses only the ordinary-Omega forms accepted by
the published Delta-produced compiler.

The source-profile constraint does not narrow the resulting compiler: this half
still implements full Omega optimization and target lowering. The Delta-built
artifact is already a viable full compiler. Rebuilding the same source with it
closes the final self-hosting edge and may optimize the compiler executable; it
does not add another language rung or another source owner.

The maintained Rust comparator lives at `source/omega-rust/omega/`. It may
continue in parallel, but is never required by the lattice. Do not place Rust
crates here.

The optimizer's durable architecture is
[`optimizer_architecture.md`](../../wiki/design_briefs/optimizer_architecture.md),
and its implementation queue is
[`TASKS_OPTIMIZER.md`](../../TASKS_OPTIMIZER.md).
