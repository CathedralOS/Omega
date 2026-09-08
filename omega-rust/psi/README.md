# Psi Compiler Workspace

Psi owns Omega source semantics: lexing, parsing, resolution, typing, checking,
proof, target-neutral optimization and production of Terminal Psi. Terminal Psi
is the self-contained portable compilation product; interpretation and native
realization consume it under their respective contracts.

Crates here must not depend on Omega backend, target, ABI, physical storage,
instruction, object or installation representations. Omega consumes verified
Terminal Psi, not a checked-tree fallback. Unsupported constructs reject rather
than acquiring an alternate source-to-native route.

## Implementation owners

- [Connected pipeline](../pipeline.md): the complete sequence and its ownership
  boundaries. Transform crates have literal `X-to-Y` names; optimizations are
  `X-to-X`, not new pre/post-optimized representations.
- [Source frontend](pipeline/README.md): lexing through typing;
  [checking](pipeline/typed-trees-to-checked-trees/README.md) owns proof,
  flow and ownership settlement.
- [Source representations](representations/README.md): durable semantic shape
  and the links between increasingly resolved forms.
- [Terminal production](compiler/terminal-production/README.md): checked trees
  → Lowered Psi → optimized Lowered Psi → Terminal Psi. Its coordinator
  sequences those transforms rather than defining another executable IR.
- [Terminal representation](representations/terminal-psi/src/lib.rs),
  [codec](semantics/terminal-codec/README.md) and
  [verifier](semantics/terminal-verifier/README.md): separate owners of the
  current module, canonical serialization and independent evidence checking.
- [Proof admission](semantics/proof-admission/README.md): product-local proof
  judgments and certificates, distinct from the
  [bootstrap proof tools](../../bootstrap/proofs/README.md).
- [Build-time evaluation](semantics/build-time-evaluation/README.md) and
  [checked interpreter](semantics/checked-interpreter/src/lib.rs): semantic
  evaluation and source-shaped reference execution.
- [Terminal interpreter](semantics/terminal-interpreter/README.md): verified
  artifact execution. Interpreter coverage does not establish native support
  or fixed-work evidence.

Cross-layer interpreter/native comparisons belong in test harnesses.
Reference interpretation remains Psi-owned and is not a native fallback.
The [Terminal specification](../../wiki/spec/terminal-psi/product.md) defines
the public product; owner notes describe implementation limits without
restricting that contract.
