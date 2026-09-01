# Semantic source owners

This tree is organized by accepted language and durable product role. A
compiler's source suffix names its immediate-predecessor implementation
language.

```text
alpha/          raw tape semantics and audited native VM seeds
beta/           strict first-order functional bootstrap calculus
gamma/          typed pure functional compiler language
delta/          closed compiler-host language
library/        core, allocation, and standard-library source
psi/            target-neutral Omega product-compiler phases
omega/          Delta-written compiler D and Omega-written compiler C
omega-rust/     maintained implementation and nonauthoritative comparator
```

The selected compiler spine is:

```text
Alpha VM seed + directly audited Beta evaluator tape
  -> gamma_compiler.beta -> gamma_compiler_bytecode.tape
  -> delta_compiler.gamma -> delta_compiler_bytecode.tape
  -> omega_compiler.delta -> omega0_compiler_bytecode.tape
  -> build.omg/main.omg -> omega_compiler_bytecode.tape
```

Every compiler artifact above the native Alpha VM is platform-independent Alpha
tape. Missing compilers remain explicit
gaps—an interpreter, host script, bridge, transpiler, or native publication
route does not stand in for one.

The derivation checker is a Beta customer beside the language chain.
`source/psi/` is an internal boundary of the Omega product compiler, not a
bootstrap language.
`omega0` and `omega` name output tapes, not source owners.

The selected rung order follows the backward audit in
[`bootstrap_minimization.md`](../wiki/design_briefs/bootstrap_minimization.md)
and candidate-C comparison in
[`bootstrap_chain_alternatives.md`](../wiki/design_briefs/bootstrap_chain_alternatives.md).
The former Beta assembler is nonauthoritative Alpha tooling under
`tools/alpha/tape-assembly/`; the former imperative Gamma rung is retained only
in Git history.

`source/omega-rust/` may build, compare, and accelerate development, but it
supplies no trusted bootstrap premise. Bootstrap invocation lives under
[`tools/bootstrap/`](../tools/bootstrap/). Bootstrap-language and checker tests
live under their subject in [`tests/`](../tests/), alongside Omega language
cases under [`tests/omega/`](../tests/omega/).

There is deliberately no generic bootstrap, assurance, canary, or
generation-owned source tree. Every retained component must reinforce one
canonical edge or product phase and state its deletion condition. Git history
is the archive.
