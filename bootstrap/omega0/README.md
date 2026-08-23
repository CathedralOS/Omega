# `bootstrap/omega0/` — Delta-built first-Omega bootstrap ownership

This directory owns the Rust-free Omega/Psi bootstrap work and the contracts and
gates for the first Delta-built Omega compiler. It is neither the production
Omega compiler nor evidence that Delta was absorbed into Omega. Delta remains
the final Greek compiler-host rung:

```text
Alpha → Beta → Gamma → Delta
                           ↓
              Omega (Delta-built, simple)
                           ↓
              Omega (Omega-built, optimized)
```

Ownership is explicit: `meaning/` contains the Rust-free meaning route,
`compiler/` contains the first-compiler profiles and source-bundle tooling, and
`gates/` contains executable acceptance checks, their untrusted encoders, and
their private fixtures.
`compiler/omega/` is a historical compatibility directory only.

## What exists here

- [`meaning/omega2gamma.beta`](meaning/omega2gamma.beta) is a Rust-free
  Omega-kernel-to-Gamma meaning translator
  written in Beta. Gamma's canonical interpreter runs the result.
- [`gates/omega-meaning.sh`](gates/omega-meaning.sh) exercises supported Omega
  samples through that route.
- [`gates/kernel-diamond.sh`](gates/kernel-diamond.sh) compares the supported kernel subset across current native
  and meaning implementations. It is a regression/coverage gate, not DDC.
- `gates/meaning-tv.sh`, `gates/input-tv.sh`, and
  `gates/translation-validation.sh` explore
  artifact-bound claims and refinement evidence.
- convergence and certificate gates run emitted evidence through the low-rung
  proof kernel and negative controls.
- [`compiler/BOOTSTRAP_PROFILES.md`](compiler/BOOTSTRAP_PROFILES.md) freezes the Delta implementation profile for Omega0
  and the first Omega console canary profile. The production-self-host profile
  remains open until a production compiler source tree exists in Omega.
- [`compiler/OMEGA0_BUNDLE.md`](compiler/OMEGA0_BUNDLE.md) specifies the canonical length-delimited multi-source
  artifact; `compiler/omega0_bundle.py` and `gates/omega0-bundle-test.sh` are untrusted packing
  and conformance tools for that format.

These are seed pieces for the first Omega compiler, not that compiler itself.
The first Delta-written frontend slice lives at
`../../compiler/delta-rs/samples/omega0-frontend.alp`: it decodes the canonical bundle,
lexes, parses, resolves, and type/count-checks the frozen O0 console program
while retaining its two boundary operands, then directly emits the canonical
terminal-Psi bytes. [`compiler/omega0-terminal-to-elf.alp`](compiler/omega0-terminal-to-elf.alp)
then emits the exact deterministic Linux x86-64 ELF directly, without a host
assembler or linker. Its gate compares that image byte-for-byte with the
production lowering and rejects malformed input before emitting any byte. This
closes the frozen O0 canary, not general Omega checking or the future full
Omega0 backend.

## Coverage boundary

The meaning route covers a growing kernel subset and must refuse unsupported
shapes loudly. Full Omega source semantics, complete terminal-Psi obligation
reconstruction, and a Delta-built compiler remain open. Exact supported cases
belong beside the scripts that gate them rather than in a drifting count here.

See:

- [Delta→first-Omega tasks](../../TASKS_BOOTSTRAP.md#delta--first-omega-readiness)
- [Omega toolchain](../../wiki/architecture/bootstrap_lattice/omega_toolchain.md)
- [Target repository structure](../../wiki/architecture/bootstrap_lattice/repository_structure.md)
