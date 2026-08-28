# Semantic source owners

This tree is organized by language or durable semantic role, not by the
implementation language that happens to realize it.

```text
alpha/          audited execution seed, assembler, and root checker
beta/           Beta language, compiler, compiler validation, and reference meaning
gamma/          Gamma language, interpreter, and type checker
delta/          Delta language, compiler source, and canonical samples
library/        core, allocation, and standard library source
omega/          complete Omega-written product compiler
  psi/          target-neutral phases through terminal Psi
omega-rust/     replaceable Rust development implementation and comparator
```

`omega-rust` may build, compare, or accelerate the product compiler, but it
does not own Psi/Omega meaning and is never trusted merely because it produced
an artifact. Repository-wide lattice convenience orchestration lives under
[`tools/lattice/`](../tools/lattice/); stable shared inputs live under
[`tests/lattice/`](../tests/lattice/).

The package library lives at `source/library/`. Temporary compiler readers may
still address that physical location while package-manager P8 completes the
ordinary package-graph route; no compatibility path exists.
