# Rung: Delta — compiler-host systems language

[Lattice overview](../bootstrap_lattice.md) | Prev: [Gamma](gamma.md) | Next: —

> **Status: WORKING ON-RAMP.** `compiler/delta-rs/` is the disposable Rust
> implementation. Its native corpus, self-hosting compiler, and Delta-to-Gamma
> meaning diamond exist today. The implementation is still being moved fully
> onto the audited bootstrap lineage.

Delta is the terminal language rung in the bootstrap spine:

```text
Alpha → Beta → Gamma → Delta
```

It adds the systems machinery needed to host the real Psi/Omega toolchain:
mutable storage, state machines, ownership and regions, effects, boundary
operations, and the compiler-scale data structures built from them. Delta may be
slow and conservatively lowered; its job is to make the production toolchain
buildable from the audited seed without Rust or another external compiler.

## Implementation

- `compiler/delta-rs/` is the current Rust on-ramp and executable specification.
- `compiler/delta-rs/samples/lowermachine.alp` is the self-hosting compiler
  written in the language.
- `DELTA_EMIT=gamma` exposes the reference meaning path. The
  `delta-meaning-diamond.sh` gate compares that path with native execution.
- `compiler/delta/` contains the checked-in bootstrap binaries produced by this
  work.

## Relationship to Psi and Omega

Delta is a bootstrap language, not the product language. Psi owns the front end
and terminal portable IR; Omega consumes terminal Psi and performs target
realization and code generation. The Psi/Omega implementations are intended to
be hosted in Delta once the rung is sufficient.

## Proofs

Delta programs may emit proof certificates, but proof checking is not a Delta
language feature or a fifth rung. The cross-cutting [proof kernel](../proof_kernel.md)
checks certificates using independent Beta and Gamma implementations.

## Trust boundary

Most Delta facilities erase into lower-rung computation. Native hardware
operations—atomics, fences, MMIO, interrupt entry, and platform runtime calls—are
explicit boundary surfaces and remain in the platform trust ledger.

## Open work

- Complete the Rust-free Delta implementation and keep its self-host fixed point.
- Make the rung sufficient to host the production Psi/Omega compiler sources.
- Continue widening the Delta-to-Gamma meaning route and its differential gates.
