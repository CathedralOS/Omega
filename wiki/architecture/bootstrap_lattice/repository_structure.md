# Bootstrap repository structure

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Product repository layout](../repository_layout.md)

The repository groups source by architectural role without scattering one role
across top-level trees. Seed-built languages live under `bootstrap/<rung>/`,
product compilers under `source/compiler/`, and cross-cutting checking under
`source/assurance/`. Compatibility symlinks and the former `compiler/`,
`apps/`, `bootstrap/rungs/`, and `bootstrap/onramps/` facades are retired.

## Canonical structure

```text
source/
  compiler/
    rust/
      apps/omega-cli/       current Rust development command
      psi/                  current Rust source/semantic producer
      omega/                current Rust target/backend producer
    omega/
      build.omg             hosted product build entrypoint
      main.omg              hosted product machine entrypoint
      psi/                  Omega-written target-neutral Psi source
      omega/                Omega-written optimizer/backend source
      source-checkpoints/   exact product closures + provisional Ωself evidence

  assurance/
    proof-kernel/
      implementations/     Beta, Gamma, and untrusted reference checkers
      tools/                elaboration, proof search, certificate utilities
      corpus/               proofs, negative controls, and seam fixtures
      gates/                soundness, cross-check, and operational-seam gates
    refinement/
      beta/                 Beta-source/Alpha-artifact reconstruction + gates
      omega-bootstrap/      bridge reconstruction + TV gate path

bootstrap/
  alpha/                    Alpha semantics, seeds, assembler, and assembler-rust/
  beta/                     Beta language, compiler, reference, and rust/
  gamma/                    Gamma language, interpreter, and type checker
  delta/                    Delta language, compiler, artifacts, and rust/
  omega-bootstrap/          Delta-built bridge compiler owner
    meaning/                Rust-free lower-rung meaning for Delta/bridge slices
    compiler/               Delta source, profiles, and source-bundle format
    gates/                  Delta→bridge and future hosted-build validation
  gates/
    corpus/                 programs shared across multiple bootstrap seams
    lattice-cache-deps/     precise cache-input manifests
```

`omega/language/` remains the one blocked relocation. Its planned destination
is `source/library/`, but package-manager P8 must first remove the hardcoded
standard-library path.

Compiler directories are named by durable role, not by every implementation
language that may temporarily occupy that role. `source/compiler/rust/` carries
the language qualifier because it is the external Rust producer. The permanent
product owners are `source/compiler/omega/psi/` and
`source/compiler/omega/omega/`: those paths are for Omega-written source and
therefore do not receive `-rs` or `-rust` suffixes. If another external-language
producer is retained, it belongs under its own explicit implementation-language
owner rather than replacing or renaming the product paths.

## Ownership rules

- `bootstrap/<rung>/` owns the canonical language definition, lattice-built
  artifacts, the smallest implementation that establishes the rung, and any
  role-local external-language producer under a plainly named child. Rust
  producers have no semantic authority merely because they are colocated.
- `source/assurance/` owns cross-cutting checking and refinement. The generic
  proof kernel is not a compiler rung or a product compiler phase.
- `bootstrap/omega-bootstrap/` owns only the Delta-written bridge, its Rust-free
  meaning route, bridge-specific contracts, and gates. It may consume product
  source and canonical Psi/Omega formats but does not own production source.
- `source/compiler/rust/` owns the current working Rust compiler as a maintained
  reference and migration producer. It is removable from bootstrap and release
  builds once the hosted compiler closes.
- `source/compiler/omega/{psi,omega}/` owns the Omega-written product source.
  The first Psi lexical checkpoint has landed; later Psi phases and the Omega
  backend remain open.
- `source/compiler/omega/source-checkpoints/` owns exact deterministic product
  closures and provisional `Ωself` censuses.
- `source/compiler/omega/{build.omg,main.omg}` owns the hosted product entrypoint.
- The Rust producer's `psi-proof-admission` crate checks product-local Psi
  judgments and has no bootstrap-lattice authority.
- Shared corpora belong at the narrowest common owner. Cross-rung fixtures live
  in `bootstrap/gates/corpus/`; package-shaped integration fixtures live in
  `tests/fixtures/packages/`.

## Reference tooling is not an ownership axis

Independent implementations may exist as references or conformance tools, but
multiplicity does not grant authority. Compiler outputs become acceptable
through lower-rooted source-to-artifact refinement, not agreement between
producers. See
[D5](decisions.md#d5--direct-checked-refinement-closes-compiler-provenance).

Beta's executable semantic reference lives at `bootstrap/beta/reference/`;
symbolic reconstruction lives at `source/assurance/refinement/beta/`. Python
and Rust are implementation details rather than ownership categories.

## Canonical ownership map

Gate scripts resolve cross-owner dependencies through
[`bootstrap/paths.sh`](../../../bootstrap/paths.sh), and
[`bootstrap/check-path-hygiene.sh`](../../../bootstrap/check-path-hygiene.sh)
rejects new sibling-relative cross-owner references.

| Responsibility | Canonical owner | Placement status |
| --- | --- | --- |
| Alpha rung and assembler | `bootstrap/alpha/` | complete |
| Alpha assembler Rust producer | `bootstrap/alpha/assembler-rust/` | complete |
| Beta rung and reference | `bootstrap/beta/` | complete |
| Beta Rust producer | `bootstrap/beta/rust/` | complete |
| Gamma rung | `bootstrap/gamma/` | complete |
| Delta rung | `bootstrap/delta/` | complete; Delta v1 remains open |
| Delta Rust producer | `bootstrap/delta/rust/` | complete |
| current Rust Psi/Omega compiler and CLI | `source/compiler/rust/` | complete |
| cross-cutting proof kernel | `source/assurance/proof-kernel/` | placement complete; assurance capabilities continue to evolve here |
| Beta-source/Alpha-artifact refinement | `source/assurance/refinement/beta/` | complete |
| bridge reconstruction and refinement | `source/assurance/refinement/omega-bootstrap/` | placement complete; bridge assurance remains open |
| shared lattice inputs | `bootstrap/gates/{corpus,lattice-cache-deps}/` | complete |
| Omega-written Psi/Omega compiler | `source/compiler/omega/{psi,omega}/` | first Psi lexical checkpoint landed |
| product source checkpoints | `source/compiler/omega/source-checkpoints/` | active |
| hosted product entrypoint | `source/compiler/omega/{build.omg,main.omg}` | active |
| standard library | `omega/language/` | move to `source/library/` blocked on P8 |

`source/` contains shipped compiler and assurance source. `bootstrap/` contains
the language spine, its role-local producers, the Delta bridge, and shared
lattice gates. `tests/` contains language canaries and package fixtures;
`tools/` contains repository maintenance utilities.
