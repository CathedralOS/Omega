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
- The Psi-aware artifact verifier reconstructs the obligations imposed by an
  exact terminal-Psi module; the [proof kernel](proof_kernel.md) independently
  checks the certificate derivations that discharge those obligations.

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

The current Rust `psi-terminal-verifier` demonstrates the artifact-aware half:
it validates canonical terminal Psi, reconstructs its exact obligation set,
rejects missing or extra evidence, and produces `VerifiedTerminalModule`. It is
not interchangeable with the generic proof kernel. The final hosted architecture
must either place an auditable reference verifier on the bootstrap spine, make
the Psi verifier emit a reconstruction derivation checked by the low kernel, or
list the Psi verifier explicitly in the trusted base.

Bootstrap hosting, native refinement evidence, and the terminal-verifier trust
closure remain execution work under P3 in [`TASKS.md`](../../../TASKS.md).
Production optimization remains outside the trusted proof kernel.
