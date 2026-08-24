# `bootstrap/omega-bootstrap/` — Delta-built bridge ownership

This directory owns the Delta-written bridge, its bridge-specific Rust-free
Omega/Psi meaning route, contracts, and gates. It may consume the product
compiler's source bundle and canonical Psi/Omega formats, but it does not own
production Psi/Omega implementation tasks or the Omega-written product
compiler. Historical `omega0` path and artifact names were transitional;
compatibility aliases do not define an Omega0 language rung. Delta remains an
independent final Greek compiler-host language:

```text
Alpha → Beta → Gamma → Delta
Delta bridge source ──[lattice-built Delta compiler]──▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
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
`compiler/{psi,omega}/` product roots.

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
  Its native differential also pins buffered byte publication across the 4 KiB
  boundary, ordering with line output and input, nonzero/implicit exit, and trap
  flushing.
- [`gates/delta-terminal-to-elf-meaning.sh`](gates/delta-terminal-to-elf-meaning.sh)
  compares the Delta backend's complete status and artifact bytes across native
  execution and the Rust-free Gamma meaning route, including malformed and
  exhausted inputs.
- [`gates/delta-source-custody-artifact.sh`](gates/delta-source-custody-artifact.sh)
  composes source bundle → `CKIR1` → deterministic ELF across native and
  lowermachine-built producer/backend paths, product behavior, exhaustive
  schema/resource teeth with a representative self-built split, and exact
  independent ELF reconstruction with byte-wide mutation controls.
- [`gates/delta-source-custody-artifact-meaning.sh`](gates/delta-source-custody-artifact-meaning.sh)
  requires the `CKIR1` producer and backend to reproduce native 0/251/252 status
  and every published byte through the persisted Beta-written Gamma route.
- [`gates/delta-o1-selfhost-composite.sh`](gates/delta-o1-selfhost-composite.sh)
  recompiles both frozen O1 compiler programs through Delta's `lowermachine`,
  composes bundle → vocabulary-28 terminal Psi → ELF, and requires exact
  terminal and image bytes for both single-source and auxiliary-trivia bundles,
  plus fail-closed semantic and exhaustion observations.
- [`gates/scalar-call-reference.sh`](gates/scalar-call-reference.sh) pins the
  product-owned, deterministic vocabulary-28 signed-`i32` scalar-`Call` fixture,
  meaning, Linux x86-64 internal-call lowering, and structural mutation teeth.
  [`gates/delta-scalar-call-frontend.sh`](gates/delta-scalar-call-frontend.sh)
  carries the implemented table-driven Delta source tranche through native and
  lowermachine-built frontends plus independent product validation. The fixture
  remains differential evidence, not bootstrap authority.
- [`compiler/omega-bootstrap-source-custody-check.alp`](compiler/omega-bootstrap-source-custody-check.alp)
  is the first checkpoint-driven, general raw-unit parser/typechecker cost
  probe. Its [contract](compiler/SOURCE_CUSTODY_FRONTEND_PROBE.md) and focused
  [native/self-built](gates/delta-source-custody-frontend.sh) and
  [Rust-free meaning](gates/delta-source-custody-meaning.sh) gates cover the
  exact `source.omg` unit, name/order independence, semantic mutations, and
  declared resources without publishing an artifact or admitting a feature to
  `Ωself`.
- [`../assurance/refinement/omega-bootstrap/`](../assurance/refinement/omega-bootstrap/)
  owns the meaning-TV, input-TV, translation-validation, and
  generated-certificate replay gates and their untrusted encoders.
- convergence and certificate gates run emitted evidence through the low-rung
  proof kernel and negative controls.
- [`compiler/BOOTSTRAP_PROFILES.md`](compiler/BOOTSTRAP_PROFILES.md) freezes the
  current Delta implementation profile, the O0/O1 Omega console canaries, and
  the bounded profile-neutral scalar-call conformance slice.
  Product checkpoint 000001 now supplies a separately hashed, mechanically
  enforced provisional `Ωself`
  [normalized-syntax/resource profile](../../compiler/source-checkpoints/profile-000001.json).
  Typed semantics, ABI/layout, lowering, Delta capacity behavior, and bridge
  costs remain open, and the profile freezes only when the final source closure
  and general bridge settle every retained or refactored-away feature.
- [`compiler/OMEGA_BOOTSTRAP_BUNDLE.md`](compiler/OMEGA_BOOTSTRAP_BUNDLE.md)
  specifies the canonical length-delimited multi-source artifact;
  `compiler/omega_bootstrap_bundle.py` and
  `gates/omega-bootstrap-bundle-test.sh` are untrusted packing and conformance
  tools for that format.

These are seed pieces for `omega-bootstrap`, not that compiler itself. The first
checkpoint-driven compositional frontend/typechecker cost probe over
`compiler/psi/source/source.omg` is measured and closed as a checker-only
claim. Its artifact tranche has selected private `CKIR1` plus direct
conservative lowering because current Terminal-Psi vocabulary 28 cannot express
the needed general structural scalar mutation and runtime indexing. That
artifact tranche remains open while its lower-rooted refinement closes. The
exact format, Delta producer/direct backend, native/self byte identity,
canonical-Gamma 0/251/252 meaning, product behavior comparison, exhaustive
resource/relation teeth, and exact reconstruction of the selected layout,
frame, templates, fixups, segments, padding, and EOF are now executable gates.
`CKIR1` is a private handoff, not Terminal Psi, a
product IR, a source dialect, or a third feature inventory. A Terminal-Psi
vocabulary change would be product work, not an assumed prerequisite here.
The backend uses three statically partitioned fixed arenas rather than general
allocation or one carrier per logical table. This is an implementation choice
inside Delta: it does not add an allocator feature or expose CKIR layout to
Omega source. The remaining obligations are listed explicitly in
`TASKS_BOOTSTRAP.md` and §10 of
[`compiler/OMEGA_BOOTSTRAP_CHECKED_IR.md`](compiler/OMEGA_BOOTSTRAP_CHECKED_IR.md).
[`compiler/omega-bootstrap-frontend.alp`](compiler/omega-bootstrap-frontend.alp)
is the canonical Delta-written frontend source. It decodes the canonical bundle,
retains bounded labels and exact source spans, validates every unit independently,
selects exactly one program-bearing unit without concatenation, lexes, parses,
and type/count-checks O0/O1's variable straight-line console body or the bounded
table-driven scalar-call lane, then emits canonical terminal-Psi bytes. The
scalar lane supports named machines, signed-`i32` parameters/results, literals,
parameter references, and forward acyclic calls. Empty, line-comment-only, and
nested-block-comment-only auxiliary units are a pre-profile transport/scanner
canary, not module semantics or `Ωself` admission. The same bounded nested
comment scanner is used inside the program unit and rejects a delimiter that
would need another source unit to close. The old Delta-sample path is a
compatibility symlink.
[`compiler/omega-bootstrap-terminal-to-elf.alp`](compiler/omega-bootstrap-terminal-to-elf.alp)
accepts the same 0–16-write profile and the bounded scalar-call terminal shape,
then emits a deterministic Linux x86-64 ELF directly without a host assembler
or linker. Focused gates compare the canonical fixtures byte-for-byte, validate
general scalar terminals with the product codec/verifier/interpreter, run the
resulting images, and reject malformed or exhausted inputs before emitting any
byte. The composite self-host gate additionally proves that the same frozen
frontend and backend still compose after both are compiled by the Delta-written
`lowermachine`. Its initial `lowermachine` executable remains a disposable Rust
on-ramp product, and Darwin assembly/signing still uses `clang` and `codesign`;
the claim is bounded dependency/behavior closure, not a Rust-free root or
general Omega checking. The inferred scalar root and process-status shim are
conformance conventions, not Omega's authored `target::ProgramEntry`. This
closes O1 and the scalar tranche, not the future `Ωself` bridge backend.

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
