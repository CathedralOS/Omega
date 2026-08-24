# Omega product source

This root owns the Omega-written target-lowering, optimization, and
artifact-emission half of the production compiler; implementation remains open.
Its source
closure is constrained to `Ωself` and compiled by `omega-bootstrap`; those
product passes are not duplicated in the bridge.

The source-profile constraint does not narrow the resulting compiler: this half
still implements full Omega optimization and target lowering. An optional
self-rebuild may optimize the compiler executable itself, but is not another
language rung or required bootstrap edge.

The current Rust implementation is explicitly transitional and lives at
`bootstrap/onramps/omega-rust/omega/`. Do not place new Rust crates here.
