# Omega product backend

This root owns product optimization, target lowering, and artifact emission in
both full Omega compiler implementations:

- `D` is written in Delta and produces the canonical `omega₀` Alpha tape;
- `C` is written in Omega and produces the canonical self-hosted `omega` Alpha
  tape when compiled by `omega₀`.

`D` and `C` implement the same complete Omega language. `D` may generate a
slow, conservatively lowered compiler executable. `C` may use a deliberately
plain subset of ordinary Omega source to make the first self-build tractable,
but that source profile does not narrow either compiler's accepted language.

The compiler executables are Alpha tapes. Native ARM64, x86-64, UEFI, and other
artifacts emitted for user programs belong to this product backend; they do not
turn the Beta, Gamma, Delta, or Omega compiler artifacts into native binaries.

The maintained Rust comparator lives at `source/omega-rust/omega/`. It may
continue in parallel, but is never required by the lattice. Do not place Rust
crates here.

The optimizer's durable architecture is
[`optimizer_architecture.md`](../../wiki/design_briefs/optimizer_architecture.md),
and its implementation queue is
[`TASKS_OPTIMIZER.md`](../../TASKS_OPTIMIZER.md).
