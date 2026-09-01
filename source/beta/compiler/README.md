# Beta assembler

This directory owns the compiler from Beta assembly text to raw Alpha tape.
The authoritative grammar and encoding are fixed by
[`../LANGUAGE.md`](../LANGUAGE.md).

The cold-start implementation is
[`beta_assembler_bytecode.tape`](beta_assembler_bytecode.tape), a 6,816-byte
platform-independent Alpha program retained directly rather than pretending
that another textual compiler precedes Beta. The readable
[`assembler.beta`](assembler.beta) source is the same assembler expressed in
Beta; `tests/beta/compiler/selfhost.sh` requires it to reproduce the direct
tape byte-for-byte.

The assembler accepts mnemonic opcodes, complete-token `rN` registers for
`N` in `0..255`, unsigned 64-bit decimal immediates, labels, comments, and the
closed `db` string encoding. It emits no host container. Current local build
scripts may stamp an emitted tape into a native Alpha seed because the seeds do
not yet load an external tape directly.

- `beta_assembler_bytecode.tape` — direct Alpha implementation and canonical
  cold-start compiler artifact.
- `assembler.beta` — readable self-host source.
Materialization and disposable builds live under `tools/bootstrap/beta/`.
Self-hosting, the independent assembler, regressions, and examples live under
`tests/beta/compiler/`.

## Retention inventory

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `beta_assembler_bytecode.tape` | Sole directly retained Alpha program implementing Beta assembly. | Replace atomically with its exact checked relation and reconstruction. |
| `assembler.beta` | Readable self-host reconstruction source. | Delete only if another equally direct reconstruction replaces it. |

The assembler is a compiler rung, not a second native implementation. Only the
Alpha VM is hand-authored per platform.
