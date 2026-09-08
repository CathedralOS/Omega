# Beta

Beta is the first trusted language above Alpha. It gives Alpha instructions
human-readable mnemonics, numeric address assertions, hexadecimal operands, and data spelling while
remaining a deliberately small imperative tape-assembly language.

[`LANGUAGE.md`](LANGUAGE.md) defines the exact deterministic relation from one
Beta source byte sequence to one raw Alpha `.tape`. The compiler is the
platform-independent [`beta_compiler_bytecode.tape`](beta_compiler_bytecode.tape),
an admitted Alpha program in the trusted bootstrap chain. Its readable
[`beta_compiler.beta`](beta_compiler.beta) source must
reconstruct that tape byte-for-byte.
The finite [compiler root audit](AUDIT.md) binds its identities,
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
[`bootstrap/0_alpha/SEMANTICS.md`](../0_alpha/SEMANTICS.md).

## Beta compiler

The admitted tape is a 1,773-byte Alpha program.
`tests/beta/compiler/reconstruction.sh` requires the readable source to
reproduce it byte-for-byte.

The compiler accepts mnemonic opcodes, complete-token lowercase hexadecimal
registers `rH`/`rHH`, `0x`-prefixed lowercase hexadecimal 64-bit words,
numeric address assertions, comments, and fixed-width `dw` data. It
emits in one pass; mnemonic rows carry their NUL-terminated operand-width lists;
and bounded source/output pointers use a documented high-register ABI. It emits
no host container. Current local build
scripts may stamp an emitted tape into a native Alpha seed because the seeds do
not yet load an external tape directly.

The minimized implementation validates source bytes while reading, retains
absolute bounded pointers, shares one hexadecimal parser, checks every authored
address assertion against the running output cursor, and drives operands from
visible NUL-terminated width lists beside each mnemonic. Alpha's specified
zero-initialized register file supplies named persistent state and constants.
The source transport admits 64 MiB so readable prior-rung expansions can pass
through Beta; emitted Alpha remains capped by the exact 16,777,212-byte seed hole.
There is no symbol table, label identity, relocation pass, hash, fixup chain, or
compressed name. Comma removal and new Alpha opcodes remain rejected because
their additional trust cost outweighs the remaining byte savings.
Retained numeric control targets repeat the human block name in a comment beside
the instruction; the corresponding address assertion names that block too.
Comments aid the finite audit but never select target identity or affect encoding.

Materialization and disposable builds live under `tools/bootstrap/beta/`.
Reconstruction, the independent compiler, regressions, and examples live under
`tests/beta/compiler/`.

## Retention inventory

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Exact Beta-source-to-Alpha-tape language relation. | Replace only with a versioned contract and synchronized compiler and edge gates. |
| `beta_compiler_bytecode.tape` | Admitted Alpha program implementing the trusted Beta compiler. | Replace atomically with its exact checked relation and reconstruction. |
| `beta_compiler.beta` | Authoritative readable self-reconstruction source. | Delete only if another equally direct reconstruction replaces it. |
| `AUDIT.md` | Finite decoded audit and source-to-tape correspondence for the admitted pair. | Replace only with a stronger audit of the exact admitted subject. |
