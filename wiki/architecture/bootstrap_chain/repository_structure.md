# Bootstrap repository structure

[Chain overview](bootstrap_chain.md) | [Standing decisions](decisions.md)

```text
bootstrap/
  alpha/                         Alpha semantics and audited native VM seeds
  beta/                          trusted imperative tape-assembly language
    compiler/
      beta_compiler.beta         self-reconstructing source
      beta_compiler_bytecode.tape admitted Alpha implementation
  gamma/                         typed scalar/effect functional language
    evaluator/
      gamma_evaluator.beta       selected direct Beta evaluator
      gamma_evaluator_bytecode.tape derived Alpha implementation
    bootstrap/concatenative/     downgraded former Gamma implementation
  delta/                         typed pure functional language
    compiler/
      delta_compiler.gamma       selected staged recursive-ADT/match compiler
    bootstrap/
      concatenative-compiler/    downgraded former Delta compiler
  epsilon/                       fixed-storage compiler-host language
    compiler/
      epsilon_compiler.delta     checked evaluator entrance
      epsilon_compiler.delta.sources ordered Delta source manifest
      representations/           syntax, checked facts, and execution values
      lexical/, parsing/         source validation and syntax construction
      checking/                  declarations, types, calls, and body judgments
      execution/                 invocation, storage, scalars, and control
  omega/
    compiler/*.epsilon           incomplete Epsilon-written Omega compiler D
    compiler.epsilon.sources canonical D member manifest

source/
  psi/                           target-neutral Omega product phases
  omega/
    build.omg, main.omg          Omega-written compiler C roots
  library/                       Omega libraries

omega-rust/                      maintained comparator, never bootstrap authority

tools/
  bootstrap/alpha/               seed selection and tape stamping
  bootstrap/beta/                Beta compiler materialization and builds
  bootstrap/paths.sh             replaceable path registry
  bootstrap/source_closure.py    byte-only Delta/Epsilon manifest materializer
  bootstrap/check-chain-hygiene.sh

tests/
  alpha/                         Alpha conformance and reference tests
  beta/                          Beta reconstruction and compiler tests
  gamma/                         Gamma evaluator tests
  bootstrap/                     cross-owner seed checks
  omega/                         Omega product language cases
```

The Gamma evaluator belongs under `bootstrap/gamma/evaluator/` because it
implements Gamma meaning and is written in Beta. Downgraded implementations
remain nested beneath the bootstrap language whose transition they document.
The selected Delta compiler path remains open.

## Naming

`.beta`, `.gamma`, `.delta`, `.epsilon`, and `.omg` identify the selected source
languages. `.tape` identifies canonical Alpha bytecode.

A compiler or evaluator owner is named by the language it accepts or executes;
its source suffix names the language implementing it:

| Owner | Current source |
| --- | --- |
| Beta compiler | `bootstrap/beta/compiler/beta_compiler.beta` |
| Gamma evaluator | `bootstrap/gamma/evaluator/gamma_evaluator.beta` |
| Delta compiler | `bootstrap/delta/compiler/delta_compiler.gamma` (selected staged implementation) |
| Epsilon evaluator | `bootstrap/epsilon/compiler/epsilon_compiler.delta.sources`; entrance `epsilon_compiler.delta` |
| Omega `D` | `bootstrap/omega/compiler.epsilon.sources` and `bootstrap/omega/compiler/*.epsilon` |
| Omega `C` | `source/omega/build.omg`, `source/omega/main.omg` |

There is no intermediate self-host owner.
Language-owned bootstrap subdirectories are explicitly nonselected and excluded
from edge inventories. Cross-owner paths are checked by
`tools/bootstrap/check-chain-hygiene.sh`.
