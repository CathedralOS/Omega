# Bootstrap repository structure

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Product repository layout](../repository_layout.md)

The repository distinguishes architectural role from implementation language.
The former flat `compiler/` inventory has been split: seed-built language rungs,
external-language on-ramps, assurance, bootstrap Omega, and the product compiler
now have separate owners. Compatibility symlinks preserve selected old entry
points without restoring their old ownership.

## Canonical structure

```text
bootstrap/
  rungs/
    alpha/                  written semantics + native seeds + Alpha assembler
    beta/                   Beta language + self-hosting compiler
      reference/            executable reference meaning + semantic fuzzing
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
      omega0/               Omega0 meaning/artifact reconstruction + TV gates

  omega0/                   work toward the Delta-built, simple first Omega compiler
    meaning/                Rust-free Omega/Psi meaning route used by Omega0
    compiler/               Delta source, bootstrap profiles, and source-bundle format
    gates/                  current Delta→Omega validation; future self-build validation

  corpus/                   programs shared across multiple bootstrap seams

compiler/
  psi/                      eventual Omega-written target-neutral Psi source
  omega/                    eventual Omega-written optimizer/backend source
```

Product-root names describe responsibility rather than host language. On-ramp
names deliberately expose their external implementation language and temporary
status. The current Rust compiler therefore does not occupy the unsuffixed
product roots.

## Ownership rules

- `bootstrap/rungs/` owns language definitions and the smallest implementation
  that establishes each rung. A program merely *written in* Beta or Gamma does
  not belong to that language directory.
- `bootstrap/onramps/` owns disposable external-language producers. On-ramps
  have no semantic authority and are removable once the corresponding
  lattice-built route closes.
- `bootstrap/assurance/` owns cross-cutting checking and refinement. The generic
  proof kernel is not a compiler rung. Its trusted checker implementations,
  untrusted automation, corpora, and integration gates must be visibly
  separated.
- `bootstrap/omega0/` owns work and artifacts toward the first Delta-built Omega
  compiler and the minimum Psi/Omega path it needs. It is not the production
  compiler and is not another language rung.
- `bootstrap/onramps/omega-rust/` owns the current working Rust compiler as an
  untrusted migration/reference producer. It is removable once the hosted
  compiler closes.
- `compiler/psi/` and `compiler/omega/` are reserved for the eventual
  Omega-written product source. Their placeholder READMEs are not a compiler
  implementation and do not freeze the bootstrap acceptance profile.
- The Rust producer's `psi-proof-kernel` checks Psi judgments and admissions;
  it is distinct from the bootstrap derivation checker under
  `bootstrap/assurance/`.
- Shared corpora belong at the narrowest common owner. A fixture used by several
  rungs or assurance seams belongs in `bootstrap/corpus/`, not in whichever gate
  happened to be written first.

## Reference tooling is not an ownership axis

There is intentionally no owner for redundant compiler implementations.
Independent implementations may exist as references or conformance tools, but multiplicity
does not grant authority. Compiler outputs become acceptable through
lower-rooted source-to-artifact refinement, not agreement between producers.
See [D5](decisions.md#d5--direct-checked-refinement-closes-compiler-provenance).

The useful contents formerly grouped under `compiler/beta-lang-py/` now have
role-based owners:

| Former content / retained responsibility | Actual role | Canonical owner |
| --- | --- | --- |
| `beta_interp.py` and semantic fuzzing | executable Beta reference meaning | `bootstrap/rungs/beta/reference/` |
| `beta_symbolic.py` and symbolic-loop checks | untrusted refinement reconstruction | `bootstrap/assurance/refinement/beta/` |
| `beta_parser.py` | shared untrusted Beta source recognition | `bootstrap/rungs/beta/reference/`, imported by refinement |

The former byte-comparison gate and `bc2.py` backend have been removed after
showing they provided no unique semantic or refinement coverage. The `bc`
cold-start edge closes only through lower-rooted source-to-artifact checking.
`compiler/beta-lang-py/` now contains compatibility forwarding entry points,
not a canonical implementation owner.

Python is an implementation detail of these tools, not their common owner.

## Canonical map and compatibility paths

Gate scripts now resolve cross-owner dependencies through
[`bootstrap/paths.sh`](../../../bootstrap/paths.sh), and
[`bootstrap/check-path-hygiene.sh`](../../../bootstrap/check-path-hygiene.sh)
rejects new named sibling-relative references. Broad moves can therefore update
one role manifest instead of rewriting every gate. The completed ownership map
and its remaining compatibility paths are:

| Canonical or transitional source | Target role |
| --- | --- |
| `bootstrap/rungs/alpha/` (compatibility: `compiler/alpha`, historical `compiler/beta`) | `bootstrap/rungs/alpha/` — complete |
| `bootstrap/rungs/beta/` (compatibility: `compiler/beta-lang`) | `bootstrap/rungs/beta/` — complete |
| `bootstrap/rungs/gamma/` (compatibility: `compiler/gamma`) | `bootstrap/rungs/gamma/` — complete |
| `bootstrap/rungs/delta/` (compatibility: `compiler/delta`, Delta samples through `compiler/delta-rs`) | `bootstrap/rungs/delta/` — complete |
| `bootstrap/onramps/delta-rust/` (compatibility: `compiler/delta-rs`) | `bootstrap/onramps/delta-rust/` — complete |
| `bootstrap/onramps/alpha-assembler-rust/` (compatibility: `compiler/beta-rs`) | `bootstrap/onramps/alpha-assembler-rust/` — complete |
| `bootstrap/onramps/beta-rust/` (compatibility: `compiler/beta-lang-rs`) | `bootstrap/onramps/beta-rust/` — complete |
| current Rust Psi/Omega compiler and CLI | `bootstrap/onramps/omega-rust/` — complete |
| `bootstrap/assurance/proof-kernel/` (compatibility: `compiler/proof-kernel`) | `bootstrap/assurance/proof-kernel/{implementations,tools,corpus,gates}/` — complete |
| Beta-source/Alpha-artifact refinement tools (compatibility entries under Alpha) | `bootstrap/assurance/refinement/beta/` — complete |
| Omega0 meaning/artifact TV encoders and gates (compatibility entries under Omega0 gates) | `bootstrap/assurance/refinement/omega0/` — complete |
| `bootstrap/omega0/` | placement under `bootstrap/omega0/{meaning,compiler,gates}/` — complete; compiler implementation remains open |
| `bootstrap/corpus/` (compatibility: `compiler/lattice-corpus`) | `bootstrap/corpus/` — complete |
| eventual Omega-written Psi/Omega compiler | `compiler/{psi,omega}/` — roots reserved; implementation open |

`compiler/` means source intended to survive in the self-hosted product;
`bootstrap/` contains both the seed-built construction and explicitly named
external-language on-ramps. No temporary Rust producer occupies an unsuffixed
product root.
