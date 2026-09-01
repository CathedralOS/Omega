# Alpha Tape assembler

This directory owns the compiler from Alpha Tape Assembly text to raw Alpha tape.
The authoritative grammar and encoding are fixed by
[`../LANGUAGE.md`](../LANGUAGE.md).

The cold-start implementation is
[`alpha_tape_assembler_bytecode.tape`](alpha_tape_assembler_bytecode.tape), a 6,816-byte
platform-independent Alpha program retained directly rather than pretending
that another textual compiler precedes it. The readable
[`assembler.alphaasm`](assembler.alphaasm) source is the same assembler expressed in
Alpha Tape Assembly; `tests/alpha/tape-assembly/compiler/reconstruction.sh`
requires it to reproduce the direct tape byte-for-byte.

The assembler accepts mnemonic opcodes, complete-token `rN` registers for
`N` in `0..255`, unsigned 64-bit decimal immediates, labels, comments, and the
closed `db` string encoding. It emits no host container. Current local build
scripts may stamp an emitted tape into a native Alpha seed because the seeds do
not yet load an external tape directly.

- `alpha_tape_assembler_bytecode.tape` — direct Alpha implementation and canonical
  cold-start compiler artifact.
- `assembler.alphaasm` — readable reconstruction source.
Materialization and disposable builds live under `tools/alpha/tape-assembly/`.
Reconstruction, the independent assembler, regressions, and examples live under
`tests/alpha/tape-assembly/compiler/`.

## Retention inventory

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `alpha_tape_assembler_bytecode.tape` | Sole directly retained Alpha program implementing Alpha Tape Assembly. | Replace atomically with its exact checked relation and reconstruction. |
| `assembler.alphaasm` | Readable reconstruction source. | Delete only if another equally direct reconstruction replaces it. |

The assembler is a tool, not a compiler rung or a second native Alpha
implementation.
