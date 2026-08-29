# Semantic source owners

This tree is organized by accepted language and durable product role. The
implementation suffix records the immediate predecessor used to implement a
compiler; it does not create another source owner.

```text
alpha/          Alpha semantics, audited native VM seeds, assembler, and checker
  assembler/    Alpha source-to-tape construction
  checker/      separate root derivation checker and its bounded gates
beta/           Beta language, reference meaning, and Alpha-written compiler
  compiler/     beta_compiler.alpha, its Alpha tape, and adjacent validation
gamma/          Gamma language and bounded Beta-written semantic components
  compiler/     owner of the future Beta-written Gamma compiler and Alpha tape
delta/          Delta language
  compiler/     owner of the future Gamma-written Delta compiler and Alpha tape
library/        core, allocation, and standard-library source
psi/            Omega-written target-neutral product-compiler phases
omega/          Delta-written compiler D and Omega-written compiler closure C
omega-rust/     maintained Rust implementation and nonauthoritative comparator
```

The only compiler spine is:

```text
Alpha VM seed
  -> beta_compiler.alpha -> beta_compiler_bytecode.tape
  -> gamma_compiler.beta -> gamma_compiler_bytecode.tape
  -> delta_compiler.gamma -> delta_compiler_bytecode.tape
  -> omega_compiler.delta -> omega0_compiler_bytecode.tape
  -> build.omg/main.omg   -> omega_compiler_bytecode.tape
```

Every artifact above the native Alpha seed is platform-independent Alpha tape.
Missing compilers remain explicit gaps; an interpreter, host script, bridge,
transpiler, or native publication route does not stand in for one.

The Alpha checker is a separate binary from the VM and assembler and remains
under `alpha/checker/` because it checks root derivations for canonical edges.
It is adjacent trust-floor infrastructure, not a compiler rung and not a
semantic producer.

`source/psi/` is an internal boundary of the Omega-written product compiler,
not a bootstrap language. `source/omega/omega_compiler.delta` will own the
Delta-written full compiler `D`; `source/omega/{build,main}.omg` root the
Omega-written compiler closure `C`. `omega0` and `omega` name the two resulting
tapes, not languages or source trees.

`source/omega-rust/` may build, compare, and accelerate development, but it does
not own Psi/Omega meaning and supplies no trusted bootstrap premise.
Repository-wide lattice invocation lives under
[`tools/lattice/`](../tools/lattice/), while Omega language cases live under
[`tests/omega/`](../tests/omega/).

There is deliberately no `bootstrap/`, `omega-bootstrap/`, `on-ramp/`,
`assurance/`, generic `canaries/`, or generation-owned source tree. Every owned
component must reinforce one canonical edge or one product-compiler phase, have
a present consumer and bounded gate, and state when it is absorbed or deleted.
Material that cannot be adapted into that shape has negative value and is
removed; Git history is the archive.

Optional library packages live at `source/library/`; none are compiler-trusted
or required to exist. Temporary compiler readers may still address that
physical location while
`OPTIONAL-STDLIB-BUILD-PROTOCOL-AND-SEMANTIC-BINDINGS` removes the remaining
physical-path and build-protocol dependencies; no compatibility path survives
that migration.
