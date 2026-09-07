# Rung: Beta

[Chain overview](../bootstrap_chain.md) | Prev: [Alpha](alpha.md) | Next: [Gamma](gamma.md)

Beta is the trusted imperative tape-assembly language above Alpha. It gives
Alpha's 21 instructions mnemonic spelling, numeric address assertions,
hexadecimal words, comments, and one fixed-word data directive. Its semantics
is the deterministic partial relation from Beta source to raw Alpha tape.

The normative contract is
[`bootstrap/beta/LANGUAGE.md`](../../../../bootstrap/beta/LANGUAGE.md). The admitted
1,773-byte compiler tape runs on Alpha, while
`bootstrap/beta/compiler/beta_compiler.beta` reconstructs that tape
byte-identically. The compiler differential and strict grammar gates live under
`tests/beta/compiler/`.
The finite root audit is published at
[`bootstrap/beta/compiler/AUDIT.md`](../../../../bootstrap/beta/compiler/AUDIT.md).

Beta's language-chain customer is the Gamma evaluator at
`bootstrap/gamma/evaluator/gamma_evaluator.beta`. Beta self-reconstruction binds
its readable compiler source to the cold-start tape; no later intermediate rung
is required to self-host.
