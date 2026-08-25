# `bootstrap/omega-bootstrap/` — Delta-built bridge ownership

This directory owns the Delta-written bridge, its bridge-specific Rust-free
Omega/Psi meaning route, contracts, and gates. It may consume the product
compiler's source bundle and canonical Psi/Omega formats, but it does not own
production Psi/Omega implementation tasks or the Omega-written product
compiler. Delta remains an independent final Greek compiler-host language:

```text
Alpha → Beta → Gamma → Delta
Delta bridge source ──[lattice-built Delta compiler]──▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

`omega-bootstrap` may itself be conservatively built and may conservatively
lower the product compiler. It must compile the `Ωself` source that implements
the product optimizer and advanced lowering, but the bridge need not implement
or run those passes itself. It treats those modules as ordinary accepted input;
the resulting production compiler contains and runs them on later compilations.
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

Some compatibility inputs retain historical filenames. They have no owner,
language, generation, or architectural role; new documentation and work use
`omega-bootstrap` exclusively.

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
  independent reference reconstruction of ELF bytes with byte-wide mutation
  controls. The separate lower-rooted CKIR1→ELF refinement checker lives under
  `bootstrap/assurance/refinement/omega-bootstrap/`.
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
- [`compiler/OMEGA_BOOTSTRAP_COMPILATION.md`](compiler/OMEGA_BOOTSTRAP_COMPILATION.md)
  specifies the next private package/source/alias envelope. It canonically
  binds bundle entries to opaque package commitments and resolver-owned logical
  module placement, with optional authored declarations required to agree,
  without treating labels as identity. Structural validation remains
  untrusted transport evidence; compilation acceptance also requires the
  independently accepted resolver/lock commitment and source-level resolution,
  visibility, root, checked-IR, and artifact joins.
  `compiler/omega_bootstrap_compilation.py` and
  `gates/omega-bootstrap-compilation-test.sh` provide the untrusted canonical
  pack/inspect path and structural mutation/resource gate. The independent
  Delta checker `compiler/omega-bootstrap-compilation-check.alp` repeats the
  bounded wire canonicality claim through native, self-built, and Rust-free
  `0`/`251`/`252` gates with empty output. It neither checks the independently
  supplied envelope SHA-256 nor grants resolver/lock authority, resolves source
  names, validates source semantics, compares CKIR, or accepts an artifact.
- [`compiler/OMEGA_BOOTSTRAP_RESOLUTION.md`](compiler/OMEGA_BOOTSTRAP_RESOLUTION.md)
  fixes the modular multi-unit frontend boundary. A Delta resolver emits the
  canonical `OMGRSW1` binding handoff; a separate lowerer consumes exact
  `OMGCOMP + OMGRSW1` through `OMGLOW1` before publishing CKIR. The same
  resolution bytes become an untrusted lower-rooted witness, while the existing
  one-unit source-custody producer remains a regression/reference path.
  [`gates/delta-resolution-handoff.sh`](gates/delta-resolution-handoff.sh)
  exhausts the standalone resolver's semantic/resource boundaries and requires
  exact native/Delta-self-built output; the separate
  [meaning gate](gates/delta-resolution-handoff-meaning.sh) pins canonical
  `0`, semantic `251`, and resource `252` observations through Gamma under a
  1 MiB elaboration ceiling. These gates close normalized resolution, not
  resolver/lock authority, digest custody, body lowering, CKIR, or ELF.
- [`compiler/omega-bootstrap-resolved-to-ckir.alp`](compiler/omega-bootstrap-resolved-to-ckir.alp)
  is the separate `OMGLOW1` consumer. It locally validates every witness family
  it uses, reparses exact resolved bodies without repeating package/name
  resolution, and emits the canonical two-package CKIR1. Its
  [native/self gate](gates/delta-resolved-to-ckir.sh) pins the exact 996-byte
  CKIR, result 70, and 17 phase-isolated relation/resource mutations; its
  [meaning gate](gates/delta-resolved-to-ckir-meaning.sh) repeats canonical
  `0`, semantic `251`, and resource `252` through Gamma under a measured
  393,216-byte elaboration ceiling. CKIR→ELF composition and lower-rooted
  `OMGRFN2` reconstruction remain separate seams.
- [`compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V2.md`](compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V2.md)
  versions that private handoff for an exact selected root and finite acyclic
  attached-machine calls. The Delta-written
  [`compiler/omega-bootstrap-resolved-to-ckir2.alp`](compiler/omega-bootstrap-resolved-to-ckir2.alp)
  and
  [`compiler/omega-bootstrap-checked-ir-v2-to-elf.alp`](compiler/omega-bootstrap-checked-ir-v2-to-elf.alp)
  close focused native and Delta-self-built production, validation,
  conservative ELF emission, exact result, and independent byte reconstruction
  for the same-module cross-source call fixture. The focused native/self-built
  [`gates/delta-resolved-to-ckir2.sh`](gates/delta-resolved-to-ckir2.sh) and
  [`gates/delta-checked-ir-v2-backend.sh`](gates/delta-checked-ir-v2-backend.sh),
  Rust-free
  [`gates/delta-resolved-to-ckir2-meaning.sh`](gates/delta-resolved-to-ckir2-meaning.sh)
  and
  [`gates/delta-checked-ir-v2-backend-meaning.sh`](gates/delta-checked-ir-v2-backend-meaning.sh),
  and complete
  [`gates/delta-role3-ckir2-composite.sh`](gates/delta-role3-ckir2-composite.sh)
  close the producer/meaning side. Lower-rooted `OMGRFN3` refinement now closes
  frame/source custody, source→role-3 witness, witness→CKIR2 tables,
  body/call/source-only-result reconstruction, CKIR/result validation, and
  CKIR2→ELF reconstruction. The final same-frame gate feeds one canonical
  10,704-byte role-3 frame to all seven executables implementing those five
  responsibilities and isolates witness, CKIR, and ELF ownership with local
  mutations. The complete versioned-call slice is now part of the canonical
  lattice; it remains bounded evidence rather than final `Ωself` admission.
- [`compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V3.md`](compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V3.md)
  adds a typed canonical constant DAG, aggregate copy, scalar `<=`, canonical
  guardless jumps, and state-edge interval custody for the selected generated-
  data tranche. The focused
  [`gates/delta-resolved-to-ckir3.sh`](gates/delta-resolved-to-ckir3.sh)
  closes native/Delta-self-built producer identity over the exact Unicode unit,
  renamed and cyclic positives, semantic negatives, and literal-resource
  boundaries. Its focused Rust-free
  [`gates/delta-resolved-to-ckir3-meaning.sh`](gates/delta-resolved-to-ckir3-meaning.sh)
  elaborates the same general lowerer once through persisted Beta and requires
  exact native/Gamma agreement for a compact typed constant DAG, aggregate
  copy, `<=`, result 70, semantic 251, and resource 252. The independent
  [`gates/delta-checked-ir-v3-backend.sh`](gates/delta-checked-ir-v3-backend.sh)
  validates the constant graph, derives layout and a private read-only image,
  and emits native/self-identical two- or three-segment ELF bytes. Its Rust-free
  [`gates/delta-checked-ir-v3-backend-meaning.sh`](gates/delta-checked-ir-v3-backend-meaning.sh)
  independently evaluates the representative CKIR3 to result 70, reconstructs
  its exact 12,288-byte three-segment ELF, and requires Gamma to reproduce that
  publication plus isolated 251/252 empty-output controls. The focused
  [`gates/delta-ckir3-composite.sh`](gates/delta-ckir3-composite.sh) runs every
  native/self producer/backend pairing over exact source frames, derives
  results 70 and 71 through the independent
  [`checked_ir_v3_reference.py`](gates/checked_ir_v3_reference.py), reconstructs
  every ELF byte through
  [`checked_elf_v3_reference.py`](gates/checked_elf_v3_reference.py), and rejects
  valid-but-mismatched CKIR/ELF pairs. Darwin does not execute the Linux image;
  exact reconstruction binds its exit shim and code to the independently
  evaluated CKIR result. The distinct
  [`OMGRFN4`](../assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V4.md)
  carrier, exact 4,497,544-byte simultaneous ceiling, and five-responsibility
  ownership split are frozen without widening earlier frames. The five lower-
  rooted CKIR3 implementations/composition remain a separate open seam.

  The meaning gates deliberately reuse one elaboration and do not duplicate
  the native/self fixture or mutation matrices. On the measured Darwin-arm64
  host, the producer observations took 68.82s/18.05s/12.12s for 0/251/252 and
  the backend's exact 12,288-byte publication took 142.52s, while backend
  rejection controls remained subsecond. A full cyclic producer observation
  took 157.75s and did not yield the required pair-shaped Gamma observation;
  it is not claimed here. Cyclic interval and artifact behavior remain covered
  by the native/self and mixed composite gates. Directly elaborating the cyclic
  Omega fixture is not used to paper over that gap: `omega2gamma` currently
  refuses its structured selected-owner array field-access path.
- [`gates/delta-two-package-composite.sh`](gates/delta-two-package-composite.sh)
  composes the actual resolver, resolved-source lowerer, and limited backend
  across native, Delta-self-built, and mixed-stage paths. It requires exact
  OMGRSW1, 996-byte CKIR, 8,192-byte ELF, result 70, independent ELF
  reconstruction, valid cross-pair rejection, and representative empty-output
  251/252 failures at every executable seam. The separate
  [Gamma composition gate](gates/delta-two-package-composite-meaning.sh) feeds
  the CKIR bytes produced by the Gamma lowerer directly to the Gamma backend
  and requires the same exact ELF.
- Lower-rooted `OMGRFN2` refinement is split by responsibility under
  [`../assurance/refinement/omega-bootstrap/`](../assurance/refinement/omega-bootstrap/):
  layer 1 checks exact framing and OMGCOMP/source custody; layer 2 independently
  reconstructs source→witness resolution; layer 3 reconstructs witness→CKIR
  declaration, layout, and root tables; layer 4 reconstructs resolved
  bodies→CKIR and computes the full source result in a companion executable
  from which CKIR and ELF readers are physically absent; and layer 5 adapts the
  complete CKIR/result and CKIR→ELF relations to the v2 frame. The lattice
  driver composes all five gates after the
  native/self-built and Rust-free producer composition. This closes the
  selected public two-package, finite, acyclic, returning artifact relation.
  It does not grant resolver/digest authority or admit a general source family
  to `Ωself`. The split is deliberate: transport, resolution, table/layout,
  body/result, and artifact claims share versioned bytes without becoming one
  verifier or a Cartesian product of fixture permutations.

These are seed pieces for `omega-bootstrap`, not that compiler itself. The first
checkpoint-driven compositional frontend/typechecker cost probe over
`compiler/psi/source/source.omg` is measured and closed as a checker-only
claim. Its artifact tranche has selected private `CKIR1` plus direct
conservative lowering because current Terminal-Psi vocabulary 28 cannot express
the needed general structural scalar mutation and runtime indexing. That first
finite, acyclic, returning artifact tranche is now closed. The
exact format, Delta producer/direct backend, native/self byte identity,
canonical-Gamma 0/251/252 meaning, product behavior comparison, exhaustive
resource/relation teeth, and exact reference reconstruction of the selected
layout, frame, templates, fixups, segments, padding, and EOF are now executable
gates. Lower-rooted source reconstruction covers declarations, types,
signatures, copy/layout, bodies, operands, terminators, transition facts,
canonical evaluation order, and an independent full source result; the
CKIR1→limited-ELF checker reconstructs the selected artifact and observation.
Valid cross-pairs isolate both joins. This bounded closure does not claim cycle,
trap, or divergence observations, nor does it admit these source families to
the still-provisional `Ωself` profile.
`CKIR1` is a private handoff, not Terminal Psi, a
product IR, a source dialect, or a third source contract. A Terminal-Psi
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
