# `bootstrap/omega-bootstrap/` — Delta-built bridge

This directory owns the Delta-written compiler that performs the one required
hosted build of production Omega. It also owns bridge-specific formats, the
Rust-free meaning route, and executable bridge gates. It does not own the
Omega-written product compiler or general product Psi/Omega work.

```text
Alpha → Beta → Gamma → Delta
Delta compiler source ──[Delta→Gamma + Gamma execution]──▶ delta compiler
Delta bridge source ──[delta compiler]───────────────────▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

Here `delta compiler` means the exact Delta-written compiler artifact published
through the canonical lower-rung execution route. Its self-built and Rust-built
forms are useful controls, but the completed required path does not depend on
the Rust on-ramp.

These names denote different things:

| Name | Kind | Obligation |
| --- | --- | --- |
| Delta v1 | independent compiler-host language | admit the complete canonical Delta-compiler and `omega-bootstrap` source closures under one contract |
| `omega-bootstrap` | compiler artifact written in Delta | accept exactly the ordinary-Omega profile `Ωself` with exact semantics |
| `Ωself` | compositional source profile, not a language rung | contain the complete Omega source closure of production `omega` |
| production `omega` | full product compiler | accept full Omega and contain the optimizer and advanced lowering |

The bridge may run slowly and lower the product compiler conservatively. It
must compile the `Ωself` modules that implement the optimizer and advanced
lowering, but it need not implement or execute those passes itself. The first
production binary contains those passes even if its own machine code is
unoptimized. A later `omega` → `omega` rebuild is optional performance and
reproducibility work, not another bootstrap rung.

## Ownership

| Path | Owns |
| --- | --- |
| [`meaning/`](meaning/) | lower-rung, Rust-free meaning for the Delta compiler and admitted bridge slices |
| [`compiler/`](compiler/) | Delta bridge source, private versioned handoffs, source-bundle contracts, and bounded historical profiles |
| [`gates/`](gates/) | focused producer, meaning, resource, composition, and regression gates plus private fixtures |
| [`../../source/assurance/refinement/omega-bootstrap/`](../../source/assurance/refinement/omega-bootstrap/) | independent lower-rooted reconstruction and source-to-artifact refinement |
| [`../../source/compiler/omega/{psi,omega}/`](../../source/compiler/omega/) | Omega-written production compiler source; not owned here |
| [`../../source/compiler/omega/source-checkpoints/`](../../source/compiler/omega/source-checkpoints/) | exact product-source closures and provisional `Ωself` evidence |

Some fixtures and compatibility inputs retain O0/O1 or `omega0` filenames.
They are historical vertical canaries, not compiler generations, language
specifications, or ancestors of `Ωself`. New architecture and work use the
role name `omega-bootstrap`.

## Current closure

The repository has working bounded slices, not the complete bridge:

| Responsibility | Current boundary | Canonical contracts |
| --- | --- | --- |
| source bundle and multi-unit custody | canonical bounded transport, generic OMGCOMP1 checking, exact Linux-x86-64/native-provider configuration custody in OMGCOMP2, and bounded Delta SHA-256 over exact raw envelopes; source spellings and expected commitments remain externally owned, so no package/lock authority follows | [`OMEGA_BOOTSTRAP_BUNDLE.md`](compiler/OMEGA_BOOTSTRAP_BUNDLE.md), [`OMEGA_BOOTSTRAP_COMPILATION.md`](compiler/OMEGA_BOOTSTRAP_COMPILATION.md), [`OMEGA_BOOTSTRAP_COMPILATION_V2.md`](compiler/OMEGA_BOOTSTRAP_COMPILATION_V2.md), [`OMEGA_BOOTSTRAP_SHA256.md`](compiler/OMEGA_BOOTSTRAP_SHA256.md) |
| generated ordinary-source custody | closed for checkpoint 000001's exact Unicode tuple through a sealed locked/offline recipe, generic provenance roles, two-run reproduction, bounded/no-publication teeth, exact OMGCOMP1 extent, CKIR3/OMGRFN4 preflight composition, and the coherent product-owned checkpoint refresh | [`OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md`](compiler/OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md), [`source checkpoint status`](../../source/compiler/omega/source-checkpoints/README.md) |
| resolution | selected public multi-unit, call, data, receiver, scalar, view, and recursive full-width arithmetic relations; OMGRSW7 is the least witness only when the complete source needs its selected `u32 in Trapping` relation | [`OMEGA_BOOTSTRAP_RESOLUTION_V7.md`](compiler/OMEGA_BOOTSTRAP_RESOLUTION_V7.md) and its versioned predecessors under [`compiler/`](compiler/) |
| checked lowering and conservative emission | CKIR14 closes one bounded recursive, pure, same-carrier full-width `u32 in Trapping` `+`/`-`/`*` tree with ordinary precedence, first-trap behavior, exact widening leaves, representative contexts, and optional inherited CKIR12 view composition; it remains a private cost slice rather than the product IR | [`OMEGA_BOOTSTRAP_CHECKED_IR_V14.md`](compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V14.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V14_BACKEND.md`](compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V14_BACKEND.md), and their versioned predecessors |
| lower-rooted reconstruction | persisted lower-rooted R1–R5 reconstruction remains closed through OMGRFN14; OMGRFN16 currently has a producer-backed independent reference scaffold and is not the current lower-rooted successor until its persisted-Beta owners pass | [`OMGCOMP_REFINEMENT_WITNESS_V14.md`](../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V14.md), provisional [`OMGCOMP_REFINEMENT_WITNESS_V16.md`](../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V16.md), and the [assurance index](../../source/assurance/refinement/omega-bootstrap/README.md) |
| product-source coverage | checkpoint 000001 has source/profile evidence; typed semantics, later phases, general lowering, and final profile remain open | [`../../source/compiler/omega/source-checkpoints/README.md`](../../source/compiler/omega/source-checkpoints/README.md) |
| compilation authority | waiting for the canonical accepted-lock/closure projection and exact envelope-commitment join | [`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md#external-contract-dependency) |

Each bounded slice measures implementation and assurance cost. It does not
admit a feature to final `Ωself`, define Delta v1, make a private CKIR into a
product IR, or grant compilation authority. Exact formats, byte counts,
resource ceilings, negative matrices, and historical results belong in the
linked contracts and beside the gates that enforce them—not in this index.

## Design boundaries

- Delta is an independent robust compiler-host language. Resembling Omega where
  cheap does not make Delta an Omega subset.
- `Ωself` contains only ordinary valid Omega. The bridge may reject excluded
  Omega features, but accepted programs retain exact Omega meaning; there is no
  private bootstrap dialect.
- The bridge may use a direct private checked IR and conservative backend. It
  need not use Terminal Psi internally.
- Product Terminal-Psi representation and lowering modules belong to the
  product source closure when the compiler links them. Standalone interpreters,
  viewers, REPLs, proof explorers, and debuggers do not belong unless imported
  by the compiler executable.
- Fixed backing, integer-offset arenas, bulk reclamation, or paged reservation
  are implementation choices until Delta specifies and admits them. A current
  buffer layout is not a language feature.
- Rust implementations are on-ramps or differential references. Authority
  comes from canonical meaning and lower-rooted source-to-artifact refinement,
  not compiler agreement or diverse double compilation.
- Unsupported source and resource exhaustion reject before artifact
  publication. A bounded probe or transport decoder is evidence, not bridge
  admission.

## Working entry points

- [`compiler/BOOTSTRAP_PROFILES.md`](compiler/BOOTSTRAP_PROFILES.md) records the
  legacy canaries and points to the source-derived provisional profile.
- [`compiler/SOURCE_CUSTODY_FRONTEND_PROBE.md`](compiler/SOURCE_CUSTODY_FRONTEND_PROBE.md)
  is the first checkpoint-driven general frontend cost probe.
- [`meaning/omega2gamma.beta`](meaning/omega2gamma.beta) and Gamma's canonical
  interpreter form the Rust-free lower-rung meaning route being widened across
  the Delta compiler and admitted bridge slices.
- [`gates/lowermachine-meaning.sh`](gates/lowermachine-meaning.sh) checks the
  actual Delta compiler through that route, including buffered byte output.
- [`../../source/assurance/refinement/omega-bootstrap/`](../../source/assurance/refinement/omega-bootstrap/)
  owns artifact-specific reconstruction and refinement gates.

The live order and acceptance conditions exist only in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md). Product Psi/Omega implementation
work exists only in [`TASKS.md`](../../TASKS.md).

See also:

- [Delta and `Ωself`](../../wiki/architecture/bootstrap_lattice/compiler_source_profile.md)
- [Psi/Omega toolchain](../../wiki/architecture/bootstrap_lattice/omega_toolchain.md)
- [Repository structure](../../wiki/architecture/bootstrap_lattice/repository_structure.md)
- [Ratified lattice decisions](../../wiki/architecture/bootstrap_lattice/decisions.md)
