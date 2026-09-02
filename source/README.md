# Semantic source owners

This tree is organized by accepted language and durable product role. A
compiler's source suffix names its immediate-predecessor implementation
language.

```text
alpha/          raw tape semantics and audited native VM seeds
beta/           trusted imperative tape-assembly language and compiler
gamma/          bounded concatenative compiler machine
delta/          typed pure functional compiler language
epsilon/        closed compiler-host language
library/        core, allocation, and standard-library source
psi/            target-neutral Omega product-compiler phases
omega/          Epsilon-written compiler D and Omega-written compiler C
omega-rust/     maintained implementation and nonauthoritative comparator
```

The selected compiler spine is:

```text
Alpha VM seed + beta_compiler_bytecode.tape
  -> gamma_evaluator.beta -> gamma_evaluator_bytecode.tape
  -> delta_compiler.gamma -> delta_compiler_bytecode.tape
  -> epsilon_compiler.delta -> epsilon_compiler_bytecode.tape
  -> omega_compiler.epsilon -> omega0_compiler_bytecode.tape
  -> build.omg/main.omg -> omega_compiler_bytecode.tape
```

Every compiler artifact above the native Alpha VM is platform-independent Alpha
tape. Missing compilers remain explicit
gaps—an interpreter, host script, bridge, transpiler, or native publication
route does not stand in for one.

The derivation checker is a Gamma customer beside the language chain.
`source/psi/` is an internal boundary of the Omega product compiler, not a
bootstrap language.
`omega0` and `omega` name output tapes, not source owners.

The selected rung order follows the backward audit in
[`bootstrap_minimization.md`](../wiki/design_briefs/bootstrap_minimization.md)
and candidate-C comparison in
[`bootstrap_chain_alternatives.md`](../wiki/design_briefs/bootstrap_chain_alternatives.md).
The imperative tape-assembly language is trusted Beta. The functional language
previously called Beta is Gamma; the former Gamma and Delta owners are now
Delta and Epsilon. The older imperative Gamma rung remains only in Git history.

`source/omega-rust/` may build, compare, and accelerate development, but it
supplies no trusted bootstrap premise. Bootstrap invocation lives under
[`tools/bootstrap/`](../tools/bootstrap/). Bootstrap-language and checker tests
live under their subject in [`tests/`](../tests/), alongside Omega language
cases under [`tests/omega/`](../tests/omega/).

There is deliberately no generic bootstrap, assurance, canary, or
generation-owned source tree. Every retained component must reinforce one
canonical edge or product phase and state its deletion condition. Git history
is the archive.
