# Psi/Omega toolchain

[Lattice overview](bootstrap_lattice.md) | [Delta language rung](rungs/delta.md)

Omega is the product language and toolchain, not another Greek bootstrap rung. Psi owns source
processing through terminal portable IR; Omega consumes that IR and performs
target realization, optimization, and native emission. Today the working
implementations are primarily Rust. The bootstrap destination has two compiler
artifacts for the same Omega language:

```text
Delta → Omega (simple, Delta-built) → Omega (optimized, Omega-built)
```

The first compiler contains the Psi source/semantic path and enough target
realization to compile conforming Omega programs. It deliberately omits advanced
optimization and may compile slowly or emit slow code. The second is the full
production compiler written in Omega and built by the first.

The distinction is architectural:

- Alpha, Beta, Gamma, and Delta form the small language chain used to build the
  first Omega compiler from the audited seed.
- The Delta-built Omega compiler is a valid self-sufficient endpoint.
- That compiler then builds the optimized Omega compiler from Omega source. The
  repeated Omega is a self-host edge, not another language rung.
- The Psi-aware artifact verifier reconstructs the obligations imposed by an
  exact terminal-Psi module; the [proof kernel](proof_kernel.md) independently
  checks the certificate derivations that discharge those obligations.

## Current repository roles

- `compiler/omega-rs/` is the current production compiler and executable reference.
- `compiler/omega/` contains Rust-free meaning and translation-validation
  experiments, including `omega2gamma.beta`.
- `compiler/delta-rs/` is the bootstrap language on-ramp growing toward building
  the simple bootstrap Omega compiler.

These are historical paths. The target ownership is `bootstrap/omega0/` for the
Delta-built first compiler and its meaning/gates, and `compiler/psi/` plus
`compiler/omega/` for the product implementation. See the
[repository structure](repository_structure.md).

Self-hosting does not by itself prove compiler correctness. A defect in bootstrap
Omega can reproduce while it builds production Omega. The value of this shape is
dependency closure: one checked self-host edge replaces a historical tower of
external implementation-language dependencies. Semantic correctness still comes
from the canonical meaning route, reconstructed proof obligations, derivation
checking, and translation validation across that edge.

The current Rust `psi-terminal-verifier` demonstrates the artifact-aware half:
it validates canonical terminal Psi, reconstructs its exact obligation set,
rejects missing or extra evidence, and produces `VerifiedTerminalModule`. It is
not interchangeable with the generic proof kernel and remains an explicit
trusted migration dependency. The final hosted architecture uses one total low-
rung semantic-ledger definition over canonical bytes. Direct evaluation or a
checked derivation of that definition establishes every deployed artifact's
ledger; Rust agreement grants no authority. Local operation denotations and
canonical goals come from restricted declarative schemas, while algebraic
reduction is untrusted and must emit a checked proof of the unchanged goal.

Bootstrap-Omega hosting, the Omega self-build, native refinement evidence, the Gamma/schema feasibility
spike, and the terminal-ledger migration remain execution work under P3 in
[`TASKS.md`](../../../TASKS.md).
Production optimization remains outside the trusted proof kernel.

The first O0 console canary now has its canonical scalar boundary lane.
Terminal-Psi vocabulary 23 carries ordered scalar parameter types on boundary
declarations and ordered scalar values on `BoundaryCall`; the checked producer,
codec, semantic schema, verifier, interpreter, and Omega abstract consumer all
preserve that lane. This closed an implementation seam, not a language ruling.

The next boundary is native realization. Omega target lowering intentionally
rejects nonempty scalar boundary calls until an exact `exit_process(i32)`
realization is supplied; the existing metadata-only port settlement is not one.
Bootstrap work must continue through the canonical representation rather than
bypass it with a private Omega0 IR or reinterpret the exit effect as an ordinary
return. `write_line` separately needs its exact structural string carrier and
custody retained through the same path.
