# Compiler lattice repository structure

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Product repository layout](../repository_layout.md)

The repository groups source by semantic owner. Bootstrap is a property of the
compiler build graph, not a source owner, and there is no standalone hosted
bridge between Delta and Omega.

```text
source/
  alpha/                 Alpha semantics and audited native VM seeds
    assembler/           Alpha-source assembler and its built tape
    checker/             universal derivation-checker source/artifact/gates
  beta/                  Beta language and reference meaning
    compiler/
      bc.beta            one Beta compiler source
      artifacts/         admitted bc tape
      cold-start/        Alpha-written construction of that tape
      validation/        exact bc source/artifact admission
  gamma/                 Gamma language
    interp.beta          canonical evaluator built by bc
    typeck.beta          canonical type checker built by bc
    reference/           optional differential implementation
  delta/                 Delta language
    compiler/
      main.alp           one canonical Delta compiler source
      artifacts/         admitted delta binary, when publication closes
      validation/        exact delta producer-edge verification and custody
    meaning/             canonical Delta-to-Gamma elaboration
    tests/               Delta language cases
  omega/                 one complete Omega-written product compiler source C
    main.omg             product compiler entry
    build.omg            product build selection
    psi/                 target-neutral phases through terminal Psi
  library/               core, allocation, and standard-library source
  omega-rust/            maintained Rust product implementation/comparator

tests/omega/              Omega language acceptance/rejection cases
tools/lattice/            replaceable convenience orchestration
```

Names in this tree identify source owners, not build generations. In
particular, `omega₀` and `omega` are two outputs from `source/omega/`; neither
gets a source directory. The unsuffixed `source/omega/` is the canonical
Omega-written implementation. The `-rust` suffix exists precisely because
`source/omega-rust/` is a parallel implementation written in another language.
Likewise, `bootstrap`, `assurance`, `refinement`, and `canaries` are not semantic
owners and do not get generic repository buckets. Evidence stays beside the
artifact it admits; product-language cases stay under `tests/omega/`.

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
- Alpha has no `compiler/` directory because its native VM seed executes Alpha
  tapes and its assembler produces them. `source/alpha/checker/` is a separate
  checker artifact used beside producer edges; it does not compile the next
  language and is not a rung.
- Gamma has no required compiler artifact. `bc` builds its Beta-written
  evaluator and type checker, and those programs give the canonical route used
  to realize and check Delta.
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
| Omega language cases | `tests/omega/` |
| non-authoritative runners | `tools/lattice/` |

Cross-owner paths are resolved through
[`tools/lattice/paths.sh`](../../../tools/lattice/paths.sh) and checked by
[`tools/lattice/check-path-hygiene.sh`](../../../tools/lattice/check-path-hygiene.sh).
