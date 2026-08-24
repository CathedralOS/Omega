# `bootstrap/omega-bootstrap/` — Delta-built bridge ownership

This directory owns the bridge-specific Rust-free Omega/Psi meaning work and
the contracts and gates for the Delta-built `omega-bootstrap` compiler. It does
not own the eventual Omega-written product compiler. Historical `omega0` path
and artifact names were transitional; compatibility aliases do not define an
Omega0 language rung. Delta remains an independent final Greek compiler-host
language:

```text
Alpha → Beta → Gamma → Delta
                           ↓
              omega-bootstrap (accepts Ωself)
                           ↓
              omega (full optimizing compiler; own binary may be conservative)
```

`omega-bootstrap` may itself be conservatively built and may conservatively
lower the product compiler. It must compile the `Ωself` source that implements
the product optimizer and advanced lowering, but need not contain those passes.
A later product self-rebuild can optimize the compiler binary; it is optional
assurance/performance work, not another rung.

Ownership is explicit: `meaning/` contains the Rust-free meaning route,
`compiler/` contains the first-compiler profiles and source-bundle tooling, and
`gates/` contains executable acceptance and conformance checks plus their
private fixtures. Cross-rung meaning/artifact obligation reconstruction lives
under `bootstrap/assurance/refinement/omega-bootstrap/`; compatibility symlinks
in `gates/` preserve historical entry points.
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
- [`gates/lowermachine-meaning.sh`](gates/lowermachine-meaning.sh) pins the
  actual Delta compiler's marker-free elaboration, exact state-table boundaries,
  bounded persistent-array carrier, canonical Gamma source ceiling, and a
  complete tiny compile whose status and output bytes equal native execution.
- [`gates/delta-terminal-to-elf-meaning.sh`](gates/delta-terminal-to-elf-meaning.sh)
  compares the Delta backend's complete status and artifact bytes across native
  execution and the Rust-free Gamma meaning route, including malformed and
  exhausted inputs.
- [`gates/delta-o1-selfhost-composite.sh`](gates/delta-o1-selfhost-composite.sh)
  recompiles both frozen O1 compiler programs through Delta's `lowermachine`,
  composes bundle → vocabulary-28 terminal Psi → ELF, and requires exact
  terminal and image bytes for both single-source and auxiliary-trivia bundles,
  plus fail-closed semantic and exhaustion observations.
- [`../assurance/refinement/omega-bootstrap/`](../assurance/refinement/omega-bootstrap/)
  owns the meaning-TV, input-TV, translation-validation, and
  generated-certificate replay gates and their untrusted encoders.
- convergence and certificate gates run emitted evidence through the low-rung
  proof kernel and negative controls.
- [`compiler/BOOTSTRAP_PROFILES.md`](compiler/BOOTSTRAP_PROFILES.md) freezes the
  current Delta implementation profile and the O0/O1 Omega console canaries.
  A provisional production `Ωself` profile can be derived and enforced once the
  exact Omega compiler source and transitive dependency manifest exists; it
  freezes only when the general bridge supplies the cost evidence used to
  settle every retained or refactored-away feature.
- [`compiler/OMEGA_BOOTSTRAP_BUNDLE.md`](compiler/OMEGA_BOOTSTRAP_BUNDLE.md)
  specifies the canonical length-delimited multi-source artifact;
  `compiler/omega_bootstrap_bundle.py` and
  `gates/omega-bootstrap-bundle-test.sh` are untrusted packing and conformance
  tools for that format.

These are seed pieces for `omega-bootstrap`, not that compiler itself.
[`compiler/omega-bootstrap-frontend.alp`](compiler/omega-bootstrap-frontend.alp)
is the canonical Delta-written frontend source. It decodes the canonical bundle,
retains bounded labels and exact source spans, validates every unit independently,
selects exactly one O1 program-bearing unit without concatenation, lexes, parses,
resolves, and type/count-checks O0 plus O1's variable straight-line console
body, then emits canonical terminal-Psi bytes. Empty and line-comment-only
auxiliary units are a pre-profile transport canary, not module semantics or an
O1 language widening. The old Delta-sample path is a
compatibility symlink.
[`compiler/omega-bootstrap-terminal-to-elf.alp`](compiler/omega-bootstrap-terminal-to-elf.alp)
accepts the same 0–16-write profile and emits a deterministic Linux x86-64 ELF
directly, without a host assembler or linker. Focused gates compare terminal
modules and images byte-for-byte with the shared product pipeline and through
lower-rung meaning, and reject malformed or exhausted inputs before emitting
any byte. The composite self-host gate additionally proves that the same frozen
frontend and backend still compose after both are compiled by the Delta-written
`lowermachine`. Its initial `lowermachine` executable remains a disposable Rust
on-ramp product, and Darwin assembly/signing still uses `clang` and `codesign`;
the claim is O1 dependency/behavior closure, not a Rust-free root or general
Omega checking. This closes O1, not the future `Ωself` bridge backend.

## Coverage boundary

The meaning route covers a growing kernel subset and must refuse unsupported
shapes loudly. Exact `Ωself` source semantics, complete terminal-Psi obligation
reconstruction, and a Delta-built bridge compiler remain open. The existing
`lowermachine.alp` now elaborates marker-free and compiles the arithmetic sample
through this route with the exact native status and 800 output bytes. This is
whole-compiler meaning evidence for the existing Delta compiler, not the future
`omega-bootstrap` compiler or `Ωself`. Exact supported cases
belong beside the scripts that gate them rather than in a drifting count here.

See:

- [Delta→omega-bootstrap tasks](../../TASKS_BOOTSTRAP.md#delta--omega-bootstrap--production-omega-readiness)
- [Delta and Ωself](../../wiki/architecture/bootstrap_lattice/compiler_source_profile.md)
- [Omega toolchain](../../wiki/architecture/bootstrap_lattice/omega_toolchain.md)
- [Target repository structure](../../wiki/architecture/bootstrap_lattice/repository_structure.md)
