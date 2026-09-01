# Rung: Delta

[Chain overview](../bootstrap_chain.md) | Prev: [Gamma](gamma.md) | Next: [Omega](../omega_toolchain.md)

Delta is the closed fixed-storage compiler-host language formerly named
Epsilon. It has finite nominal data, arrays and bounded views, checked scalar
operations, deterministic state-machine control, and sealed byte I/O. It has no
packages, heap, proof language, dependent types, or implicit host services.

The normative contract is
[`source/delta/LANGUAGE.md`](../../../../source/delta/LANGUAGE.md). The
Gamma-written compiler source at `source/delta/compiler/delta_compiler.gamma`
is incomplete and has no canonical tape. Delta's sole language-chain customer
is the first full Omega compiler closure `D` at
`source/omega/omega_compiler.delta`.

Delta does not need to compile itself. Its feature ledger admits a facility only
for `D` or a measured reduction in the complete chain.
