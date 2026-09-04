# Semantic source owners

This tree is organized by accepted language and durable product role. A
compiler's source suffix names its immediate-predecessor implementation
language.

```text
alpha/          raw tape semantics and audited native VM seeds
beta/           trusted imperative tape-assembly language and compiler
gamma/          typed scalar/effect functional bootstrap language
delta/          typed pure functional compiler language
epsilon/        closed compiler-host language
library/        core, allocation, and standard-library source
psi/            target-neutral Omega product-compiler phases
omega/          Epsilon-written compiler D and Omega-written compiler C
```

The maintained Rust implementation and nonauthoritative comparator lives at
the repository-root sibling [`omega-rust/`](../omega-rust/), outside this
eventual self-hosted source tree.

The selected compiler spine is:

```text
Alpha VM seed + beta_compiler_bytecode.tape
  -> gamma_evaluator.beta -> gamma_evaluator_bytecode.tape
  -> Gamma-authored staged source transformers
  -> Delta compiler edge (open)
  -> epsilon_compiler.delta -> canonical Delta -> ... -> epsilon_compiler_bytecode.tape
  -> omega_compiler.epsilon -> canonical Epsilon -> ... -> omega0_compiler_bytecode.tape
  -> build.omg/main.omg -> canonical Epsilon -> ... -> omega_compiler_bytecode.tape
```

Beta is the only source language that encodes Alpha instructions. Gamma is
currently evaluated directly by Beta rather than compiled. Higher-rung
transformers must still publish canonical source receipts in the immediately
supported language.
Final compiler artifacts remain platform-independent Alpha tapes where the edge
produces one. Gamma instead has an explicit direct-evaluation edge. Missing
higher compilers remain explicit gaps; host scripts, downgraded compilers, and
native publication routes do not stand in for them.

The former concatenative Gamma implementation is retained under
`source/gamma/bootstrap/concatenative/`; the former concatenative-Gamma-written
Delta compiler is under `source/delta/bootstrap/concatenative-compiler/`.
Neither is a selected edge.

The derivation checker remains an intended Gamma customer beside the language chain.
`source/psi/` is an internal boundary of the Omega product compiler, not a
bootstrap language.
`omega0` and `omega` name output tapes, not source owners.

The selected rung order follows the backward audit in
[`bootstrap_minimization.md`](../wiki/design_briefs/bootstrap_minimization.md)
and candidate-C comparison in
[`bootstrap_chain_alternatives.md`](../wiki/design_briefs/bootstrap_chain_alternatives.md).
The imperative tape-assembly language is trusted Beta. Typed scalar/effect Gamma
is the first functional source-transformer rung, followed by richer Delta and
the fixed-storage Epsilon compiler host.

`omega-rust/` may build, compare, and accelerate development, but it
supplies no trusted bootstrap premise. Bootstrap invocation lives under
[`tools/bootstrap/`](../tools/bootstrap/). Bootstrap-language and checker tests
live under their subject in [`tests/`](../tests/), alongside Omega language
cases under [`tests/omega/`](../tests/omega/).

There is deliberately no generic bootstrap, assurance, canary, or
generation-owned source tree. Every retained component must reinforce one
canonical edge or product phase and state its deletion condition. Git history
is the archive.
