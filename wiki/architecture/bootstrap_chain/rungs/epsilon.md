# Rung: Epsilon

[Chain overview](../bootstrap_chain.md) | Prev: [Delta](delta.md) | Next: [Omega](../omega_toolchain.md)

Epsilon is the closed fixed-storage compiler-host language above Delta. It has
finite nominal data, arrays and bounded views, checked scalar
operations, deterministic state-machine control, and sealed byte I/O. It has no
packages, heap, proof language, dependent types, or implicit host services.

The normative contract is
[`bootstrap/4_epsilon/LANGUAGE.md`](../../../../bootstrap/4_epsilon/LANGUAGE.md). The
Delta-written evaluator closure selected by
`bootstrap/4_epsilon/compiler/epsilon_compiler.delta.sources` is incomplete and
has no final composed artifact. Its small `epsilon_compiler.delta` entrance
leads into concept-owned checking and execution areas; see the
[source guide](../../../../bootstrap/4_epsilon/compiler/README.md).
Epsilon's sole language-chain customer
is the first full Omega compiler closure `D` selected by
`bootstrap/5_omega/compiler.epsilon.sources`.

Epsilon does not compile itself or own an Alpha backend. Its feature ledger
admits a facility only for D or a measured reduction in the complete chain.
