# Rung: Epsilon

[Chain overview](../bootstrap_chain.md) | Prev: [Delta](delta.md) | Next: [Omega](../omega_toolchain.md)

Epsilon is the closed fixed-storage compiler-host language above Delta. It has
finite nominal data, arrays and bounded views, checked scalar
operations, deterministic state-machine control, and sealed byte I/O. It has no
packages, heap, proof language, dependent types, or implicit host services.

The normative contract is
[`source/epsilon/LANGUAGE.md`](../../../../source/epsilon/LANGUAGE.md). The
Delta-written compiler source at `source/epsilon/compiler/epsilon_compiler.delta`
is incomplete and has no canonical tape. Epsilon's sole language-chain customer
is the first full Omega compiler closure `D` at
`source/omega/omega_compiler.epsilon`.

Epsilon does not need to compile itself. Its feature ledger admits a facility only
for `D` or a measured reduction in the complete chain.
