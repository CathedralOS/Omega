# Semantic source owners

This tree is organized by language or durable semantic role, not by the
implementation language that happens to realize it.

```text
alpha/          audited VM seeds, Alpha assembler, and separate root checker
  assembler/    Alpha source-to-tape construction
  checker/      derivation-checker source, artifact, gates, and implementations
beta/           Beta language and reference meaning
  compiler/     bc source/artifact, Alpha cold start, and adjacent validation
gamma/          Beta-written canonical interpreter and type checker
delta/          Delta language
  compiler/     canonical compiler source/artifact and adjacent validation
  meaning/      canonical Delta-to-Gamma meaning route
library/        core, allocation, and standard library source
psi/            Omega-written target-neutral phases through terminal Psi
omega/          Omega-written Terminal-Psi consumer and product root
omega-rust/     replaceable Rust development implementation and comparator
```

`omega-rust` may build, compare, or accelerate the product compiler, but it
does not own Psi/Omega meaning and is never trusted merely because it produced
an artifact. Repository-wide lattice convenience orchestration lives under
[`tools/lattice/`](../tools/lattice/). Omega language cases live under
[`tests/omega/`](../tests/omega/); the compiler chain owns no private copy.

There is deliberately no `bootstrap/`, `omega-bootstrap/`, `assurance/`, or
generic `canaries/` source owner. A compiler, checker, meaning route, artifact,
or validation rule lives with the language or artifact whose responsibility it
implements. `omega₀` and `omega` are two builds of the same source closure
rooted at `source/omega/build.omg`, not two source trees.

The Alpha checker is a separate binary from the Alpha VM/assembler and remains
under `alpha/checker/` because it is part of the trust floor. It does not build
Beta and is not inserted into the compiler spine. Gamma similarly needs no
published compiler binary: `bc` builds the canonical Beta-written Gamma
evaluator and type checker, which then realize Delta through its declared
meaning route.

The package library lives at `source/library/`. Temporary compiler readers may
still address that physical location while package-manager P8 completes the
ordinary package-graph route; no compatibility path exists.
