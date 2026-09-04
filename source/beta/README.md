# Beta

Beta is the first trusted language above Alpha. It gives Alpha instructions
human-readable mnemonics, numeric address assertions, hexadecimal operands, and data spelling while
remaining a deliberately small imperative tape-assembly language.

[`LANGUAGE.md`](LANGUAGE.md) defines the exact deterministic relation from one
Beta source byte sequence to one raw Alpha `.tape`. The compiler is the
platform-independent [`compiler/beta_compiler_bytecode.tape`](compiler/beta_compiler_bytecode.tape),
an admitted Alpha program in the trusted bootstrap chain. Its readable
[`compiler/beta_compiler.beta`](compiler/beta_compiler.beta) source must
reconstruct that tape byte-for-byte.
The finite [compiler root audit](compiler/AUDIT.md) binds its identities,
decoded Alpha inventory, control flow, memory profile, and independent
source-to-tape correspondence.

```text
audited native Alpha VM
  + beta_compiler_bytecode.tape
  + program.beta
    -> program.tape
```

The compiler is written in Beta and reconstructs its admitted tape
byte-for-byte. The checked-in tape is the cold-start implementation; the
reconstruction makes its source-level behavior reviewable without introducing
a second platform-native compiler.

Gamma's evaluator is written in Beta, so the Alpha-to-Beta and Beta-to-Gamma
edges both depend on this language judgment. Successful compilation yields
Alpha tape whose execution is governed by
[`source/alpha/SEMANTICS.md`](../alpha/SEMANTICS.md).

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Exact Beta-source-to-Alpha-tape language relation. | Replace only with a versioned contract and synchronized compiler and edge gates. |
| `compiler/` | Admitted compiler tape and its self-reconstruction source. | Replace only atomically with an equally direct trusted Beta implementation. |
| `compiler/beta_compiler.beta` | Authoritative readable self-reconstruction source for the Beta compiler. | Delete only if another exact reconstruction replaces it. |
| `compiler/beta_compiler_bytecode.tape` | Cold-start Alpha implementation of the trusted Beta compiler. | Replace atomically with its source, reconstruction, and checked relation. |

Compiler tests and examples live under `tests/beta/compiler/`; host
materialization lives under `tools/bootstrap/beta/`.
