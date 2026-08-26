# Bootstrap repository structure

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Product repository layout](../repository_layout.md)

The repository distinguishes architectural role from implementation language.
The former flat `compiler/` inventory has been split: seed-built language rungs,
external-language on-ramps, assurance, the bootstrap bridge, and the product compiler
now have separate owners. The top-level compatibility symlinks and forwarding
facade have been retired; `compiler/` owns product compiler source and its
versioned source-checkpoint evidence.

## Canonical structure

```text
bootstrap/
  rungs/
    alpha/                  written semantics + native seeds + Alpha assembler
    beta/                   Beta language + self-hosting compiler
      reference/            untrusted executable semantic reference + fuzzing
    gamma/                  Gamma language, interpreter, and type checker
    delta/                  Delta language + lattice-built compiler

  onramps/
    alpha-assembler-rust/   disposable/reference Alpha assembler producer
    beta-rust/              disposable/reference Beta producer
    delta-rust/             disposable/reference Delta producer
    omega-rust/
      apps/omega-cli/       current Rust development command
      psi/                  current Rust source/semantic producer
      omega/                current Rust target/backend producer

  assurance/
    proof-kernel/
      implementations/
        beta/               low-rung logical/equality checker
        gamma/              independently written checker implementation
        reference/          untrusted executable references
      tools/                elaboration, proof search, certificate utilities
      corpus/               proofs, negative controls, and seam fixtures
      gates/                soundness, cross-check, and operational-seam gates
    refinement/
      beta/                 Beta-source/Alpha-artifact reconstruction + gates
      omega-bootstrap/      bridge reconstruction + TV gate path

  omega-bootstrap/          Delta-built bridge compiler owner
    meaning/                Rust-free Omega/Psi meaning route used by the bridge
    compiler/               Delta source, bootstrap profiles, and source-bundle format
    gates/                  current Delta→bridge and future hosted-build validation

  corpus/                   programs shared across multiple bootstrap seams

compiler/
  psi/                      eventual Omega-written target-neutral Psi source
  omega/                    eventual Omega-written optimizer/backend source
  source-checkpoints/       exact product closures + provisional Ωself evidence

apps/
  omega-compiler/           hosted product compiler entrypoint
```

Product-root names describe responsibility rather than host language. On-ramp
names deliberately expose their external implementation language and temporary
status. The current Rust compiler therefore does not occupy the unsuffixed
product roots. Conversely, `compiler/psi/` and `compiler/omega/` need no
`-omega` suffix: they are the permanent product-role roots, and their source is
governed by the ordinary-Omega `Ωself` profile. An implementation-language
suffix marks an on-ramp, not every source directory.

## Ownership rules

- `bootstrap/rungs/` owns canonical language definitions, lattice-built
  artifacts, and the smallest implementation that establishes each rung. A rung
  becomes frozen or immutable only after its declared closure gates pass;
  reopening it is an explicit versioned change. A program merely *written in*
  Beta or Gamma does not belong to that language directory, and adjacent
  reference/fuzz tooling has no authority merely because it is role-local.
- `bootstrap/onramps/` owns external-language producers that are disposable
  from the required bootstrap and trust closure. “Disposable” does not mean
  abandoned: an on-ramp may remain maintained as a differential comparator.
  On-ramps have no semantic authority.
- `bootstrap/assurance/` owns cross-cutting checking and refinement. The generic
  proof kernel is not a compiler rung. Its trusted checker implementations,
  untrusted automation, corpora, and integration gates must be visibly
  separated.
- `bootstrap/omega-bootstrap/` owns only the Delta-written bridge, its Rust-free
  meaning route, bridge-specific contracts, and gates. It may consume product
  source and canonical Psi/Omega formats, but it does not own production Psi or
  Omega implementation work. Compatibility filenames have no architectural
  role. The bridge accepts `Ωself`, is not the production compiler, and is not
  another language rung.
- `bootstrap/onramps/omega-rust/` owns the current working Rust compiler as an
  untrusted migration/reference producer. It is removable from bootstrap and
  release builds once the hosted compiler closes, even if retained in the
  repository for differential bug finding.
- `compiler/psi/` and `compiler/omega/` own Omega-written product source.
  `compiler/psi/` contains the first source-to-token checkpoint;
  `compiler/omega/` remains an open product owner, not a Rust migration root.
  These names survive the hosted transition because they identify product
  responsibilities rather than the compiler that happened to build them.
- `compiler/source-checkpoints/` owns exact deterministic product-source
  closures and distinct provisional `Ωself` censuses.
- `apps/omega-compiler/` owns the hosted product compiler entrypoint.
- The Rust producer's `psi-proof-admission` crate checks product-local Psi
  judgments and admission policy. It is distinct from the generic bootstrap
  derivation checker under `bootstrap/assurance/` and has no bootstrap-lattice
  authority.
- Shared corpora belong at the narrowest common owner. A fixture used by several
  rungs or assurance seams belongs in `bootstrap/corpus/`, not in whichever gate
  happened to be written first.

## Reference tooling is not an ownership axis

There is intentionally no owner for redundant compiler implementations.
Independent implementations may exist as references or conformance tools, but multiplicity
does not grant authority. Compiler outputs become acceptable through
lower-rooted source-to-artifact refinement, not agreement between producers.
See [D5](decisions.md#d5--direct-checked-refinement-closes-compiler-provenance).

Beta's untrusted executable semantic reference and parser live at
`bootstrap/rungs/beta/reference/`; symbolic reconstruction lives at
`bootstrap/assurance/refinement/beta/`. Python is an implementation detail, not
an ownership category or reason for a product-root facade. The retired
comparison compiler and old paths are recorded in D5 and Git history rather
than in this canonical placement map.

## Canonical ownership map

Gate scripts now resolve cross-owner dependencies through
[`bootstrap/paths.sh`](../../../bootstrap/paths.sh), and
[`bootstrap/check-path-hygiene.sh`](../../../bootstrap/check-path-hygiene.sh)
rejects new named sibling-relative references. Broad moves can therefore update
one role manifest instead of rewriting every gate. Placement status below says
whether a responsibility has reached its canonical directory; it does not claim
that the language, compiler, or assurance work in that directory is finished.

| Responsibility | Canonical owner | Placement status |
| --- | --- | --- |
| Alpha rung and assembler | `bootstrap/rungs/alpha/` | complete |
| Beta rung | `bootstrap/rungs/beta/` | complete |
| Gamma rung | `bootstrap/rungs/gamma/` | complete |
| Delta rung | `bootstrap/rungs/delta/` | complete; Delta v1 remains open |
| Delta Rust on-ramp | `bootstrap/onramps/delta-rust/` | complete |
| Alpha assembler Rust on-ramp | `bootstrap/onramps/alpha-assembler-rust/` | complete |
| Beta Rust on-ramp | `bootstrap/onramps/beta-rust/` | complete |
| current Rust Psi/Omega compiler and CLI | `bootstrap/onramps/omega-rust/` | complete |
| cross-cutting proof kernel | `bootstrap/assurance/proof-kernel/{implementations,tools,corpus,gates}/` | complete |
| Beta-source/Alpha-artifact refinement | `bootstrap/assurance/refinement/beta/` | complete |
| bridge meaning/artifact reconstruction and gates | `bootstrap/assurance/refinement/omega-bootstrap/` | complete; bridge assurance remains open |
| shared lattice corpus | `bootstrap/corpus/` | complete |
| Omega-written Psi/Omega compiler | `compiler/{psi,omega}/` | first Psi lexical checkpoint landed; remaining phases open |
| product compiler closure/profile checkpoints | `compiler/source-checkpoints/` | active |
| hosted product compiler entrypoint | `apps/omega-compiler/` | active |

`compiler/` means Omega-written source intended to survive in the production
compiler;
`bootstrap/` contains both the seed-built construction and explicitly named
external-language on-ramps. No temporary Rust producer occupies an unsuffixed
product root.
