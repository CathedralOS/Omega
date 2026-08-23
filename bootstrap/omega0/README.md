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
`gates/` contains executable acceptance and conformance checks plus their
private fixtures. Cross-rung meaning/artifact obligation reconstruction lives
under `bootstrap/assurance/refinement/omega0/`; compatibility symlinks in `gates/`
preserve historical entry points.
The former top-level compatibility directory was retired so this bootstrap
compiler cannot be confused with either the current Rust on-ramp or the
reserved `compiler/{psi,omega}/` product roots.

## What exists here

- [`meaning/omega2gamma.beta`](meaning/omega2gamma.beta) is a Rust-free
  Omega-kernel-to-Gamma meaning translator
  written in Beta. Gamma's canonical interpreter runs the result.
- [`gates/omega-meaning.sh`](gates/omega-meaning.sh) exercises supported Omega
  samples through that route.
- [`gates/kernel-diamond.sh`](gates/kernel-diamond.sh) compares the supported kernel subset across current native
  and meaning implementations. It is a regression/coverage gate.
- [`gates/delta-terminal-to-elf-meaning.sh`](gates/delta-terminal-to-elf-meaning.sh)
  compares the Delta backend's complete status and artifact bytes across native
  execution and the Rust-free Gamma meaning route, including malformed and
  exhausted inputs.
- [`../assurance/refinement/omega0/`](../assurance/refinement/omega0/) owns the
  meaning-TV, input-TV, translation-validation, and generated-certificate replay
  gates and their untrusted encoders.
- convergence and certificate gates run emitted evidence through the low-rung
  proof kernel and negative controls.
- [`compiler/BOOTSTRAP_PROFILES.md`](compiler/BOOTSTRAP_PROFILES.md) freezes the Delta implementation profile for Omega0
  and the first Omega console canary profile. The production-self-host profile
  remains open until a production compiler source tree exists in Omega.
- [`compiler/OMEGA0_BUNDLE.md`](compiler/OMEGA0_BUNDLE.md) specifies the canonical length-delimited multi-source
  artifact; `compiler/omega0_bundle.py` and `gates/omega0-bundle-test.sh` are untrusted packing
  and conformance tools for that format.

These are seed pieces for the first Omega compiler, not that compiler itself.
[`compiler/omega0-frontend.alp`](compiler/omega0-frontend.alp) is the canonical
Delta-written frontend source. It decodes the canonical bundle, lexes, parses,
resolves, and type/count-checks O0 plus O1's variable straight-line console
body, then emits canonical terminal-Psi bytes. The old Delta-sample path is a
compatibility symlink. [`compiler/omega0-terminal-to-elf.alp`](compiler/omega0-terminal-to-elf.alp)
accepts the same 0–16-write profile and emits a deterministic Linux x86-64 ELF
directly, without a host assembler or linker. Focused gates compare terminal
modules and images byte-for-byte with the shared product pipeline and through
lower-rung meaning, and reject malformed or exhausted inputs before emitting
any byte. This closes O1, not general Omega checking or the future full Omega0
backend.

## Coverage boundary

The meaning route covers a growing kernel subset and must refuse unsupported
shapes loudly. Full Omega source semantics, complete terminal-Psi obligation
reconstruction, and a Delta-built compiler remain open. Exact supported cases
belong beside the scripts that gate them rather than in a drifting count here.

See:

- [Delta→first-Omega tasks](../../TASKS_BOOTSTRAP.md#delta--first-omega-readiness)
- [Omega toolchain](../../wiki/architecture/bootstrap_lattice/omega_toolchain.md)
- [Target repository structure](../../wiki/architecture/bootstrap_lattice/repository_structure.md)
