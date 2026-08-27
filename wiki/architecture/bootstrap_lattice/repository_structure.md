# Bootstrap repository structure

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Product repository layout](../repository_layout.md)

The repository groups source by semantic owner. Canonical languages and product
compiler halves are direct children of `source/`; replaceable external or
bridge implementations live under `source/on-ramp/`; cross-owner proof joins
are named explicitly instead of being hidden in generic `compiler`,
`bootstrap`, or `assurance` buckets.

## Canonical structure

```text
source/
  alpha/                       Alpha semantics, seeds, and assembler
  beta/                        Beta language, compiler, and reference meaning
  gamma/                       Gamma language, interpreter, and type checker
  delta/                       Delta language, compiler, and canonical artifacts
  psi/                         Omega-written target-neutral product compiler
    build.omg                  Psi package declaration
    generated/                 generated semantic tables
    lex/ source/ tokens/       current landed source-to-token checkpoint
  omega/                       Omega-written target realization and product owner
    build.omg                  hosted product build/composition entrypoint
    main.omg                   hosted product machine entrypoint
    source-checkpoints/        exact product closures + provisional Ωself evidence

  proof-kernel/
    implementations/           Beta, Gamma, and untrusted reference checkers
    tools/                     elaboration, proof search, certificate utilities
    corpus/                    proofs, negative controls, and seam fixtures
    gates/                     soundness, cross-check, and operational-seam gates
  refinement/
    alpha-beta/                Beta-source/Alpha-artifact reconstruction + gates
    delta-omega-bootstrap/     bridge reconstruction + TV gate path

  on-ramp/
    rust/
      apps/omega-cli/       current Rust development command
      psi/                  current Rust source/semantic producer
      omega/                current Rust target/backend producer
    omega-bootstrap/        Delta-built bridge compiler owner
    meaning/                Rust-free lower-rung meaning for Delta/bridge slices
    compiler/               Delta source, profiles, and source-bundle format
    gates/                  Delta→bridge and future hosted-build validation

tests/
  lattice/
    corpus/                 programs shared across multiple bootstrap seams
    lattice-cache-deps/     precise cache-input manifests

tools/
  bootstrap/               lattice orchestration and canonical path gates
```

`omega/language/` remains the one blocked relocation. Its planned destination
is `source/library/`, but package-manager P8 must first remove the hardcoded
standard-library path.

Canonical directories are named by durable role, not by every implementation
language that may temporarily occupy that role. `source/on-ramp/rust/` carries
the language qualifier because it is the temporary external Psi/Omega product
implementation. The permanent product owners are `source/psi/` and
`source/omega/`; if another
external-language producer is retained, it belongs under its own explicit
on-ramp owner rather than replacing or renaming those paths.

## Ownership rules

- `source/<rung>/` owns the canonical language definition, lattice-built
  artifacts, and the smallest implementation that establishes the rung.
  External-language producers live under `source/on-ramp/` and gain no
  semantic authority from being useful during construction.
- `source/proof-kernel/` owns cross-cutting proof checking; it is not a compiler
  rung or product compiler phase. `source/refinement/` owns explicit checked
  joins whose two endpoints have different semantic owners.
- `source/on-ramp/omega-bootstrap/` owns only the Delta-written bridge, its Rust-free
  meaning route, bridge-specific contracts, and gates. It may consume product
  source and canonical Psi/Omega formats but does not own production source.
- `source/on-ramp/rust/` owns the current working Rust compiler as a maintained
  reference and migration producer. It is removable from bootstrap and release
  builds once the hosted compiler closes.
- `source/psi/` and `source/omega/` own the two Omega-written product halves.
  Psi ends at terminal Psi; Omega begins by consuming terminal Psi and owns
  target realization. The first Psi lexical checkpoint has landed; later Psi
  phases and the Omega backend remain open.
- `source/omega/source-checkpoints/` owns exact deterministic product
  closures and provisional `Ωself` censuses.
- `source/omega/{build.omg,main.omg}` owns the hosted product entrypoint. Its
  declared `psi` dependency points at the sibling `source/psi/` owner.
  Compilation and source-checkpoint inspection both reconcile that dependency
  through `build.omg`; no nested compatibility path exists.
- The Rust producer's `psi-proof-admission` crate checks product-local Psi
  judgments and has no bootstrap-lattice authority.
- Shared corpora belong at the narrowest common owner. Cross-rung fixtures live
  in `tests/lattice/corpus/`; package-shaped integration fixtures live in
  `tests/fixtures/packages/`.

## Reference tooling is not an ownership axis

Independent implementations may exist as references or conformance tools, but
multiplicity does not grant authority. Compiler outputs become acceptable
through lower-rooted source-to-artifact refinement, not agreement between
producers. See
[D5](decisions.md#d5--direct-checked-refinement-closes-compiler-provenance).

Beta's executable semantic reference lives at `source/beta/reference/`;
symbolic reconstruction lives at `source/refinement/alpha-beta/`. Python
and Rust are implementation details rather than ownership categories.

## Canonical ownership map

Gate scripts resolve cross-owner dependencies through
[`tools/bootstrap/paths.sh`](../../../tools/bootstrap/paths.sh), and
[`tools/bootstrap/check-path-hygiene.sh`](../../../tools/bootstrap/check-path-hygiene.sh)
rejects new sibling-relative cross-owner references.

| Responsibility | Canonical owner | Placement status |
| --- | --- | --- |
| Alpha rung and assembler | `source/alpha/` | complete |
| Beta rung and reference | `source/beta/` | complete |
| Gamma rung | `source/gamma/` | complete |
| Delta rung | `source/delta/` | complete; Delta v1 remains open |
| current Rust Psi/Omega compiler and CLI | `source/on-ramp/rust/` | complete |
| cross-cutting proof kernel | `source/proof-kernel/` | placement complete; assurance capabilities continue to evolve here |
| Beta-source/Alpha-artifact refinement | `source/refinement/alpha-beta/` | complete |
| bridge reconstruction and refinement | `source/refinement/delta-omega-bootstrap/` | placement complete; bridge assurance remains open |
| shared lattice inputs | `tests/lattice/{corpus,lattice-cache-deps}/` | complete |
| Omega-written Psi/Omega compiler | `source/{psi,omega}/` | first Psi lexical checkpoint landed |
| product source checkpoints | `source/omega/source-checkpoints/` | active |
| hosted product entrypoint | `source/omega/{build.omg,main.omg}` | active |
| standard library | `omega/language/` | move to `source/library/` blocked on P8 |

`source/` contains the semantic spine, product compiler, proof services, and
clearly marked on-ramps. `tests/` contains language canaries, package fixtures,
and shared lattice inputs; `tools/` contains repository maintenance and
bootstrap orchestration.
