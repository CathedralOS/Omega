# Psi/Omega toolchain

[Lattice overview](bootstrap_lattice.md) | [Delta language rung](rungs/delta.md)

Omega is the product toolchain, not another bootstrap rung. Psi owns source
processing through terminal portable IR; Omega consumes that IR and performs
target realization, optimization, and native emission. Today the working
implementations are primarily Rust. The bootstrap destination is to host them in
Delta.

The distinction is architectural:

- Alpha, Beta, Gamma, and Delta form the language chain used to rebuild the
  toolchain from the audited seed.
- Psi and Omega are the real compiler products built by that chain.
- The [proof kernel](proof_kernel.md) independently checks certificates emitted
  alongside terminal Psi and lower artifacts.

## Current repository roles

- `omega-rs/` is the current production compiler and executable reference.
- `compiler/omega/` contains Rust-free meaning and translation-validation
  experiments, including `omega2gamma.beta`.
- `compiler/delta-rs/` is the bootstrap language on-ramp that is growing toward
  hosting the production compiler.

Self-hosting does not by itself prove compiler correctness. It removes external
toolchain dependencies and gives the project a reproducible path from the audited
seed. Semantic correctness comes from the canonical meaning route, proof
obligations, and independent certificate checking.

## Open work

- Move the full Psi/Omega implementation onto Delta without weakening the
  canonical terminal-Psi contract.
- Emit and check per-compilation refinement evidence for native artifacts.
- Keep production optimization outside the trusted proof kernel.
