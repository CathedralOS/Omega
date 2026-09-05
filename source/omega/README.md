# Omega product compiler

This package is the Omega-owned half of the final self-hosted compiler. The
sibling [`../psi/`](../psi/) package owns target-neutral source processing,
checking, proof, and Terminal Psi. This package consumes Terminal Psi and owns
provider selection, target realization, ABI, artifact emission, and the product
entrypoint.

[`build.omg`](build.omg) and [`main.omg`](main.omg) are the current roots of the
Omega-written compiler closure C. The build binds the ordinary
`alpha_bootstrap` target alongside native targets and depends on Psi and the
standard library through ordinary package declarations.

The first compiler D is a separate Epsilon-written bootstrap implementation
under [`../../bootstrap/omega/`](../../bootstrap/omega/). Interpreted D compiles
this final closure to `omega0`; `omega0` then recompiles the same closure to
`omega`. Rust under [`../../omega-rust/`](../../omega-rust/) remains a
nonauthoritative development comparator.

```text
interpreted bootstrap/omega D + source/{psi,omega,library} -> omega0
omega0 + source/{psi,omega,library}                        -> omega
```

Implementation work is tracked in [`../../TASKS.md`](../../TASKS.md), while the
trust-chain edge is tracked in
[`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
