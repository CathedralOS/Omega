# `compiler/omega/` — Rust-free first-Omega meaning and refinement experiments

This directory is historical placement for Rust-free Omega/Psi bootstrap work.
It is neither the production Omega compiler nor evidence that Delta was absorbed
into Omega. Delta remains the final Greek compiler-host rung:

```text
Alpha → Beta → Gamma → Delta
                           ↓
              Omega (Delta-built, simple)
                           ↓
              Omega (Omega-built, optimized)
```

Under the target repository structure, this work moves to
`bootstrap/omega0/meaning/`; future Delta source for the first Omega compiler
moves to `bootstrap/omega0/compiler/`.

## What exists here

- `omega2gamma.beta` is a Rust-free Omega-kernel-to-Gamma meaning translator
  written in Beta. Gamma's canonical interpreter runs the result.
- `omega-meaning.sh` exercises supported Omega samples through that route.
- `kernel-diamond.sh` compares the supported kernel subset across current native
  and meaning implementations. It is a regression/coverage gate, not DDC.
- `meaning-tv.sh`, `input-tv.sh`, and `translation-validation.sh` explore
  artifact-bound claims and refinement evidence.
- convergence and certificate gates run emitted evidence through the low-rung
  proof kernel and negative controls.
- `BOOTSTRAP_PROFILES.md` freezes the Delta implementation profile for Omega0
  and the first Omega console canary profile. The production-self-host profile
  remains open until a production compiler source tree exists in Omega.
- `OMEGA0_BUNDLE.md` specifies the canonical length-delimited multi-source
  artifact; `omega0_bundle.py` and `omega0-bundle-test.sh` are untrusted packing
  and conformance tools for that format.

These are seed pieces for the first Omega compiler, not that compiler itself.
The first Delta-written vertical slice now lives at
`../delta-rs/samples/omega0-frontend.alp`: it decodes the canonical bundle,
lexes, parses, resolves, and type/count-checks the frozen O0 console program
while retaining its two boundary operands, then directly emits the canonical
terminal-Psi bytes. The emitted fixture is gated through lower-rung meaning,
shared decode/verification, deterministic native images, and runnable Linux
behavior. General Omega checking and a Delta-written native artifact backend do
not exist yet.

## Coverage boundary

The meaning route covers a growing kernel subset and must refuse unsupported
shapes loudly. Full Omega source semantics, complete terminal-Psi obligation
reconstruction, and a Delta-built compiler remain open. Exact supported cases
belong beside the scripts that gate them rather than in a drifting count here.

See:

- [Delta→first-Omega tasks](../../TASKS_BOOTSTRAP.md#delta--first-omega-readiness)
- [Omega toolchain](../../wiki/architecture/bootstrap_lattice/omega_toolchain.md)
- [Target repository structure](../../wiki/architecture/bootstrap_lattice/repository_structure.md)
