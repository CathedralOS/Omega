# Omega product source

This root owns the Omega-written target-lowering, optimization, and
artifact-emission half of the production compiler; implementation remains open.
Its source closure deliberately uses only the ordinary-Omega forms accepted by
the published Delta-produced compiler.

The source-profile constraint does not narrow the resulting compiler: this half
still implements full Omega optimization and target lowering. An optional
self-rebuild may optimize the compiler executable itself, but is not another
language rung or required bootstrap edge.

The current Rust implementation is explicitly transitional and lives at
`source/omega-rust/omega/`. Do not place new Rust crates here.

The optimizer's durable architecture is
[`optimizer_architecture.md`](../../wiki/design_briefs/optimizer_architecture.md),
and its implementation queue is
[`TASKS_OPTIMIZER.md`](../../TASKS_OPTIMIZER.md).
