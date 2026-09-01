# Beta compiler

This directory owns the compiler from Beta text to raw Alpha tape.
The authoritative grammar and encoding are fixed by
[`../LANGUAGE.md`](../LANGUAGE.md).

The admitted cold-start implementation is
[`beta_compiler_bytecode.tape`](beta_compiler_bytecode.tape), a 6,816-byte
platform-independent Alpha program at the first compiler edge. The readable
[`beta_compiler.beta`](beta_compiler.beta) source is the same assembler expressed in
Beta; `tests/beta/compiler/reconstruction.sh`
requires it to reproduce the direct tape byte-for-byte.

The compiler accepts mnemonic opcodes, complete-token `rN` registers for
`N` in `0..255`, unsigned 64-bit decimal immediates, labels, comments, and the
closed `db` string encoding. It emits no host container. Current local build
scripts may stamp an emitted tape into a native Alpha seed because the seeds do
not yet load an external tape directly.

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
