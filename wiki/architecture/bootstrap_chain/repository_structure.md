# Bootstrap repository structure

[Chain overview](bootstrap_chain.md) | [Standing decisions](decisions.md)

```text
source/
  alpha/                         Alpha semantics and audited native VM seeds
  beta/                          strict first-order functional calculus
  gamma/                         typed pure functional language
  delta/                         fixed-storage compiler-host language
    compiler/
      delta_compiler.gamma       incomplete Gamma-written Delta compiler
  psi/                           target-neutral Omega product phases
  omega/
    omega_compiler.delta         incomplete Delta-written Omega compiler D
    build.omg, main.omg          Omega-written compiler C roots
  library/                       Omega libraries
  omega-rust/                    maintained comparator, never bootstrap authority

tools/
  alpha/tape-assembly/           off-chain readable Alpha tape tooling
  bootstrap/alpha/               seed selection and tape stamping
  bootstrap/paths.sh             replaceable path registry
  bootstrap/check-chain-hygiene.sh

tests/
  alpha/                         Alpha conformance/reference and tape-tool tests
  bootstrap/                     cross-owner seed checks
  omega/                         Omega product language cases
```

The future Beta evaluator belongs under `source/beta/evaluator/` because it is
the direct implementation of Beta meaning. The future Gamma compiler belongs
under `source/gamma/compiler/`. Empty directories are not retained merely to
reserve those paths.

## Naming

`.alphaasm` identifies off-chain Alpha Tape Assembly. `.beta`, `.gamma`,
`.delta`, and `.omg` identify the selected source languages. `.tape` identifies
canonical Alpha bytecode.

A compiler owner is named by the language it accepts; its source suffix names
the language implementing it:

| Owner | Future/current source |
| --- | --- |
| Gamma compiler | `source/gamma/compiler/gamma_compiler.beta` |
| Delta compiler | `source/delta/compiler/delta_compiler.gamma` |
| Omega `D` | `source/omega/omega_compiler.delta` |
| Omega `C` | `source/omega/build.omg`, `source/omega/main.omg` |

There is no retired Epsilon source owner, intermediate self-host owner, generic bootstrap
source bucket, or compatibility compiler. Cross-owner paths are checked by
`tools/bootstrap/check-chain-hygiene.sh`.
