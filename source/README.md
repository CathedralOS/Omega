# Semantic source owners

This tree is organized by language or durable semantic role, not by the
implementation language that happens to realize it.

```text
alpha/          audited execution seed and Alpha-owned assembler
beta/           Beta language, compiler source, artifacts, and reference meaning
gamma/          Gamma language, interpreter, and type checker
delta/          Delta language, compiler source, and canonical samples
library/        core, allocation, and standard library source
psi/            Omega-written target-neutral product compiler
omega/          Omega-written target realization and product entrypoint
omega-rust/     replaceable Rust development implementation and comparator
proof-kernel/   cross-language certificate checking
refinement/     checks over named source/artifact edges
```

`omega-rust` may build, compare, or accelerate the product compiler, but it
does not own Psi/Omega meaning and is never trusted merely because it produced
an artifact. Repository-wide lattice convenience orchestration lives under
[`tools/bootstrap/`](../tools/bootstrap/); its shared corpora and cache
inputs live under [`tests/lattice/`](../tests/lattice/).

The package library lives at `source/library/`. Temporary compiler readers may
still address that physical location while package-manager P8 completes the
ordinary package-graph route; no compatibility path exists.
