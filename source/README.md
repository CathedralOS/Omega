# Semantic source owners

This tree is organized by accepted language and durable product role. A
compiler's source suffix names its immediate-predecessor implementation
language.

```text
alpha/          raw tape semantics, audited native VM seeds, and checker
beta/           textual Alpha assembly and the direct assembler tape
  compiler/     assembler.beta and beta_assembler_bytecode.tape
gamma/          Gamma language and Beta-written compiler
  compiler/     gamma_compiler.beta and gamma_compiler_bytecode.tape
delta/          Delta language and Gamma-written compiler
  compiler/     delta_compiler.gamma and adjacent incomplete edge work
epsilon/        Epsilon language and Delta-written compiler
  compiler/     epsilon_compiler.delta and adjacent incomplete edge work
library/        core, allocation, and standard-library source
psi/            target-neutral Omega product-compiler phases
omega/          Epsilon-written compiler D and Omega-written compiler C
omega-rust/     maintained implementation and nonauthoritative comparator
```

The only compiler spine is:

```text
Alpha VM seed
  -> beta_assembler_bytecode.tape
  -> gamma_compiler.beta -> gamma_compiler_bytecode.tape
  -> delta_compiler.gamma -> delta_compiler_bytecode.tape
  -> epsilon_compiler.delta -> epsilon_compiler_bytecode.tape
  -> omega_compiler.epsilon -> omega0_compiler_bytecode.tape
  -> build.omg/main.omg -> omega_compiler_bytecode.tape
```

Every artifact above the native Alpha VM is platform-independent Alpha tape.
The Beta assembler is materialized from its raw tape when needed; no second
native assembler binary is retained. Missing upper compilers remain explicit
gaps—an interpreter, host script, bridge, transpiler, or native publication
route does not stand in for one.

The Alpha checker is a service beside the language chain. `source/psi/` is an
internal boundary of the Omega product compiler, not a bootstrap language.
`omega0` and `omega` name output tapes, not source owners.

`source/omega-rust/` may build, compare, and accelerate development, but it
supplies no trusted bootstrap premise. Bootstrap invocation lives under
[`tools/bootstrap/`](../tools/bootstrap/). Bootstrap-language and checker tests
live under their subject in [`tests/`](../tests/), alongside Omega language
cases under [`tests/omega/`](../tests/omega/).

There is deliberately no generic bootstrap, assurance, canary, or
generation-owned source tree. Every retained component must reinforce one
canonical edge or product phase and state its deletion condition. Git history
is the archive.
