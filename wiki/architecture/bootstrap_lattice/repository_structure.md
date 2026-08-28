# Compiler lattice repository structure

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Product repository layout](../repository_layout.md)

The repository groups source by semantic owner. There is no standalone hosted
bridge between Delta and Omega.

```text
source/
  alpha/                 Alpha semantics, seeds, assembler, and root checker
    checker/             universal derivation checker and its tests
  beta/                  Beta language, reference meaning, and gates
    compiler/            compiler source, artifact, cold start, and validation
  gamma/                 Gamma language, interpreter, and type checker
  delta/                 Delta language, compiler, meaning, tests, and artifacts
    compiler/            canonical compiler source and adjacent validation
      artifacts/         admitted compiler artifacts, when publication closes
      validation/        exact producer-edge verification and custody
    meaning/             canonical lower-rung elaboration
    tests/               Delta language cases
  omega/                 complete Omega-written product compiler
    psi/                 target-neutral phases through terminal Psi
  library/               core, allocation, and standard-library source
  omega-rust/             temporary Rust product implementation/comparator

tests/lattice/            shared cross-rung inputs
tests/omega/              Omega language acceptance/rejection cases
tools/lattice/            replaceable convenience orchestration
```

## Ownership rules

- `source/<rung>/` owns that rung's language, compiler source, canonical
  meaning, and artifacts.
- `source/delta/meaning/` owns the lower-rung elaboration needed to publish the
  Delta compiler. It is not a separate compiler stage.
- `source/omega/` owns the complete Omega-written product compiler. Its
  `psi/` subtree ends at terminal Psi; the rest consumes terminal Psi and owns
  target realization. Psi is a compiler phase boundary, not a repository-level
  language rung.
- `source/omega-rust/` is a maintained implementation and migration aid. Its
  location grants no bootstrap or semantic authority.
- `source/alpha/checker/` owns the universal derivation checker. It is part of
  the Alpha trust floor, not another language rung.
- The artifact being admitted owns its validation. For example,
  `source/beta/compiler/validation/` reconstructs the Beta compiler's
  source-to-artifact edge; there is no generic cross-rung dumping ground.
- `tests/lattice/` owns shared inputs, not compiler stages or trust decisions.
- `tests/omega/` owns product-language cases; it is not a bootstrap artifact.
- `tools/lattice/` may invoke the chain. A script must not parse, resolve,
  lower, discover source, manufacture evidence, or otherwise become a hidden
  compiler stage.

The package library lives at `source/library/`. Package-manager work still owns
replacement of temporary physical-path readers with package-graph resolution.

## Direct product path

Let `C` be the exact ordinary-Omega compiler source closure, deliberately
authored with a conservative feature subset:

```text
Alpha → Beta → Gamma → Delta-produced compiler
Delta-produced compiler + C → omega₀
omega₀ + the same C            → omega
```

`omega₀` is the first artifact of the product compiler, not a separately owned
bridge. Its conservative code quality is permissible; its accepted semantics
are not approximate. The second edge must use the same source closure so it is
a rebuild of one compiler, not an untracked generation change.

## Canonical ownership map

| Responsibility | Canonical owner |
| --- | --- |
| language rungs | `source/{alpha,beta,gamma,delta}/` |
| Delta lower-rung meaning | `source/delta/meaning/` |
| Omega-written compiler | `source/omega/` |
| current Rust comparator | `source/omega-rust/` |
| root proof checking | `source/alpha/checker/` |
| Beta compiler and its admission | `source/beta/compiler/` |
| Delta compiler, artifacts, and admission | `source/delta/compiler/` |
| language libraries | `source/library/` |
| shared lattice inputs | `tests/lattice/` |
| Omega language cases | `tests/omega/` |
| non-authoritative runners | `tools/lattice/` |

Cross-owner paths are resolved through
[`tools/lattice/paths.sh`](../../../tools/lattice/paths.sh) and checked by
[`tools/lattice/check-path-hygiene.sh`](../../../tools/lattice/check-path-hygiene.sh).
