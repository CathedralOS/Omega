# Bootstrap repository structure

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Product repository layout](../repository_layout.md)

The repository must distinguish architectural role from implementation
language. The current flat `compiler/` tree predates that distinction: language
rungs, Rust on-ramps, Python reference tools, the proof kernel, bootstrap Omega
experiments, and the production compiler appear as peers. That is an inventory,
not a sound ownership model.

## Target structure

```text
bootstrap/
  rungs/
    alpha/                  written semantics + native seeds + Alpha assembler
    beta/                   Beta language + self-hosting compiler
    gamma/                  Gamma language, interpreter, and type checker
    delta/                  Delta language + lattice-built compiler

  onramps/
    alpha-assembler-rust/   disposable/reference Alpha assembler producer
    beta-rust/              disposable/reference Beta producer
    delta-rust/             disposable/reference Delta producer

  assurance/
    proof-kernel/
      implementations/
        beta/               low-rung logical/equality checker
        gamma/              independently written checker implementation
        reference/          untrusted executable references
      tools/                elaboration, proof search, certificate utilities
      corpus/               proofs, negative controls, and seam fixtures
      gates/                soundness, cross-check, and operational-seam gates
    refinement/             source/artifact obligation reconstruction + gates

  omega0/                   Delta-built, simple first Omega compiler
    meaning/                Rust-free Omega/Psi meaning route used by Omega0
    compiler/               Delta source, bootstrap profiles, and source-bundle format
    gates/                  Delta→Omega and Omega self-build validation

  corpus/                   programs shared across multiple bootstrap seams

compiler/
  psi/                      production target-neutral Psi implementation
  omega/                    production target/backend Omega implementation
```

The names describe responsibility. They deliberately do not promise that the
implementation is Rust, Python, or any other language. During migration an
on-ramp may retain an implementation-language suffix, but no standing
architecture depends on that suffix.

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
- `bootstrap/omega0/` owns the first Delta-built Omega artifact and the minimum
  Psi/Omega path it needs. It is not the production compiler and is not another
  language rung.
- `compiler/psi/` and `compiler/omega/` own the product implementation. The
  product-specific `psi-proof-kernel` checks Psi judgments and admissions; it is
  distinct from the bootstrap derivation checker under `bootstrap/assurance/`.
- Shared corpora belong at the narrowest common owner. A fixture used by several
  rungs or assurance seams belongs in `bootstrap/corpus/`, not in whichever gate
  happened to be written first.

## DDC is not a repository role

There is intentionally no `diversity/` or DDC branch. Independent
implementations may exist as references or conformance tools, but multiplicity
does not grant authority. Compiler outputs become acceptable through
lower-rooted source-to-artifact refinement, not agreement between producers.
See [D5](decisions.md#d5--checked-refinement-not-ddc-closes-compiler-provenance).

The useful contents currently grouped under `compiler/beta-lang-py/` have three
different destinations:

| Current content | Actual role | Target owner |
| --- | --- | --- |
| `beta_interp.py` and semantic fuzzing | executable Beta reference meaning | `bootstrap/rungs/beta/reference/` or `bootstrap/assurance/refinement/beta/` |
| `beta_symbolic.py` and symbolic-loop checks | untrusted refinement reconstruction | `bootstrap/assurance/refinement/beta/` |
| `beta_parser.py` | shared untrusted Beta source recognition | narrowest Beta reference/refinement owner needing it |
| compiler portion of `bc2.py` | optional differential backend | retain only for unique diagnostics |

The former byte-comparison DDC gate has been removed. It is not waiting on the
`bc` cold-start refinement edge: that edge closes only through lower-rooted
source-to-artifact checking.

Python is an implementation detail of these tools, not their common owner.

## Migration map

Gate scripts now resolve cross-owner dependencies through
[`bootstrap/paths.sh`](../../../bootstrap/paths.sh), and
[`bootstrap/check-path-hygiene.sh`](../../../bootstrap/check-path-hygiene.sh)
rejects new named sibling-relative references. Broad moves may therefore update
one role manifest instead of rewriting every gate. Migrate by ownership, keeping
temporary wrappers where needed:

| Canonical or transitional source | Target role |
| --- | --- |
| `bootstrap/rungs/alpha/` (compatibility: `compiler/alpha`, historical `compiler/beta`) | `bootstrap/rungs/alpha/` — complete |
| `compiler/beta-lang/` | `bootstrap/rungs/beta/` |
| `compiler/gamma/` | `bootstrap/rungs/gamma/` |
| lattice-built Delta sources/artifacts in `compiler/delta*/` | `bootstrap/rungs/delta/` |
| `compiler/beta-rs/`, `compiler/beta-lang-rs/`, Rust portion of `compiler/delta-rs/` | `bootstrap/onramps/`, separated by produced role |
| `compiler/proof-kernel/` + Gamma checker sources | `bootstrap/assurance/proof-kernel/` |
| refinement scripts spread across `alpha/`, `omega/`, and Python tools | `bootstrap/assurance/refinement/` |
| `compiler/omega/` bootstrap experiments | `bootstrap/omega0/` |
| `compiler/lattice-corpus/` | `bootstrap/corpus/` |
| `compiler/psi-rs/`, `compiler/omega-rs/` | `compiler/psi/`, `compiler/omega/` |

The migration is complete when `compiler/` means the product compiler,
`bootstrap/` means how that product is rebuilt from the seed, and no directory
is grouped solely by the host language of temporary tooling.
