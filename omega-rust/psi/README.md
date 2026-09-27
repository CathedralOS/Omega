# Psi source processing and execution

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
  [checking](pipeline/04_typed-trees-to-checked-trees/README.md) owns proof,
  flow and ownership settlement.
- [Source representations](representations/README.md): durable semantic shape
  and the links between increasingly resolved forms.
- [Terminal production](terminal-production/README.md): checked trees
  → Lowered Psi → optimized Lowered Psi → Terminal Psi. Its coordinator
  sequences those transforms rather than defining another executable IR.
- [Terminal representation](representations/terminal-psi/src/lib.rs),
  [codec](representations/terminal-psi/codec/README.md) and
  [verifier](representations/terminal-psi/verification/README.md): separate owners of the
  current module, canonical serialization and independent evidence checking.
- [Source checking](checking/src/program_validation.rs): declaration, call,
  value and proof-contract judgments reused by checking and interpretation.
- [Source proof](proof/src/lib.rs): obligation construction and proof search,
  with independent mathematical checking in its admission package.
- [Proof admission](proof/admission/README.md): product-local proof
  judgments and certificates, distinct from the
  [bootstrap proof tools](../../bootstrap/proofs/README.md).
- [Build-time evaluation](interpreter/build-time-evaluation/README.md) and
  [checked interpreter](interpreter/checked-interpreter/src/interpreter.rs): semantic
  evaluation and source-shaped reference execution.
- [Terminal interpreter](interpreter/terminal-interpreter/README.md): verified
  artifact execution. Interpreter coverage does not establish native support
  or fixed-work evidence.

There is no separate compiler driver in this directory. The pipeline transforms,
source checking, proof services, interpreters and Terminal production each have
their own entrance. Terminal encoding, verification and fuel contracts are
independent packages beside their representation, not unrelated semantics utilities.

Cross-layer interpreter/native comparisons belong in test harnesses.
Reference interpretation remains Psi-owned and is not a native fallback.
The [Terminal specification](../../wiki/spec/terminal-psi/product.md) defines
the public product; owner notes describe implementation limits without
restricting that contract.
