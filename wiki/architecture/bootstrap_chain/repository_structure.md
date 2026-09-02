# Bootstrap repository structure

[Chain overview](bootstrap_chain.md) | [Standing decisions](decisions.md)

```text
source/
  alpha/                         Alpha semantics and audited native VM seeds
  beta/                          trusted imperative tape-assembly language
    compiler/
      beta_compiler.beta         self-reconstructing source
      beta_compiler_bytecode.tape admitted Alpha implementation
  gamma/                         bounded concatenative compiler machine
    evaluator/
      gamma_evaluator.beta       in-progress Beta-written evaluator
    compiler/
      gamma_compiler.gamma       selected Gamma-to-Beta compiler
      gamma_compiler.beta        canonical self-expansion receipt
      gamma_compiler_bytecode.tape composed native compiler
  delta/                         typed pure functional language
  epsilon/                       fixed-storage compiler-host language
    compiler/
      epsilon_compiler.delta     incomplete Delta-written Epsilon compiler
  psi/                           target-neutral Omega product phases
  omega/
    omega_compiler.epsilon       incomplete Epsilon-written Omega compiler D
    build.omg, main.omg          Omega-written compiler C roots
  library/                       Omega libraries
  omega-rust/                    maintained comparator, never bootstrap authority

tools/
  bootstrap/alpha/               seed selection and tape stamping
  bootstrap/beta/                Beta compiler materialization and builds
  bootstrap/paths.sh             replaceable path registry
  bootstrap/check-chain-hygiene.sh

tests/
  alpha/                         Alpha conformance and reference tests
  beta/                          Beta reconstruction and compiler tests
  gamma/                         Gamma evaluator tests
  bootstrap/                     cross-owner seed checks
  omega/                         Omega product language cases
```

The Gamma evaluator belongs under `source/gamma/evaluator/` because it
implements Gamma meaning and is written in Beta. The future Delta compiler
belongs under `source/delta/compiler/`. Empty directories are not retained
merely to reserve those paths.

## Naming

`.beta`, `.gamma`, `.delta`, `.epsilon`, and `.omg` identify the selected source
languages. `.tape` identifies canonical Alpha bytecode.

A compiler owner is named by the language it accepts; its source suffix names
the language implementing it:

| Owner | Future/current source |
| --- | --- |
| Beta compiler | `source/beta/compiler/beta_compiler.beta` |
| Gamma evaluator | `source/gamma/evaluator/gamma_evaluator.beta` |
| Delta compiler | `source/delta/compiler/delta_compiler.gamma` |
| Epsilon compiler | `source/epsilon/compiler/epsilon_compiler.delta` |
| Omega `D` | `source/omega/omega_compiler.epsilon` |
| Omega `C` | `source/omega/build.omg`, `source/omega/main.omg` |

There is no intermediate self-host owner, generic bootstrap source bucket, or
compatibility compiler. Cross-owner paths are checked by
`tools/bootstrap/check-chain-hygiene.sh`.
