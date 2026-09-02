# Beta compiler

This directory owns the compiler from Beta text to raw Alpha tape.
The authoritative grammar and encoding are fixed by
[`../LANGUAGE.md`](../LANGUAGE.md).

The admitted cold-start implementation is
[`beta_compiler_bytecode.tape`](beta_compiler_bytecode.tape), a 1,792-byte
platform-independent Alpha program at the first compiler edge. The readable
[`beta_compiler.beta`](beta_compiler.beta) source is the same assembler expressed in
Beta; `tests/beta/compiler/reconstruction.sh`
requires it to reproduce the direct tape byte-for-byte.

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
The source transport admits 16 MiB so readable prior-rung expansions can pass
through Beta; emitted Alpha remains capped by the exact 1,048,572-byte seed hole.
There is no symbol table, label identity, relocation pass, hash, fixup chain, or
compressed name. Comma removal and new Alpha opcodes remain rejected because
their additional trust cost outweighs the remaining byte savings.

- `beta_compiler_bytecode.tape` - direct Alpha implementation and canonical
  cold-start compiler artifact.
- `beta_compiler.beta` - readable reconstruction source.
Materialization and disposable builds live under `tools/bootstrap/beta/`.
Reconstruction, the independent compiler, regressions, and examples live under
`tests/beta/compiler/`.

## Retention inventory

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `beta_compiler_bytecode.tape` | Admitted Alpha program implementing the trusted Beta compiler. | Replace atomically with its exact checked relation and reconstruction. |
| `beta_compiler.beta` | Authoritative readable self-reconstruction source. | Delete only if another equally direct reconstruction replaces it. |

Beta and this compiler are a trusted rung. The retained Alpha tape is the
cold-start implementation, while the Beta source fixes its readable recursive
identity.
