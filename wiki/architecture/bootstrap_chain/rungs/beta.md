# Rung: Beta

[Chain overview](../bootstrap_chain.md) | Prev: [Alpha](alpha.md) | Next: [Gamma](gamma.md)

Beta is the trusted imperative tape-assembly language above Alpha. It gives
Alpha's 21 instructions mnemonic spelling, labels, hexadecimal words, comments,
and one fixed-word data directive. Its semantics is the deterministic partial
relation from Beta source to raw Alpha tape.

The normative contract is
[`source/beta/LANGUAGE.md`](../../../../source/beta/LANGUAGE.md). The admitted
1,792-byte compiler tape runs on Alpha, while
`source/beta/compiler/beta_compiler.beta` reconstructs that tape
byte-identically. The compiler differential and strict grammar gates live under
`tests/beta/compiler/`.

Beta's language-chain customer is the Gamma evaluator at
`source/gamma/evaluator/gamma_evaluator.beta`. Beta self-reconstruction binds
its readable compiler source to the cold-start tape; no later intermediate rung
is required to self-host.
