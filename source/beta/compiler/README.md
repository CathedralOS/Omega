# Beta assembler

This directory owns the compiler from Beta assembly text to raw Alpha tape.
The authoritative grammar and encoding are fixed by
[`../LANGUAGE.md`](../LANGUAGE.md).

The cold-start implementation is
[`beta_assembler_bytecode.tape`](beta_assembler_bytecode.tape), a 6,816-byte
platform-independent Alpha program retained directly rather than pretending
that another textual compiler precedes Beta. The readable
[`assembler.beta`](assembler.beta) source is the same assembler expressed in
Beta; `selfhost.sh` requires it to reproduce the direct tape byte-for-byte.

The assembler accepts mnemonic opcodes, complete-token `rN` registers for
`N` in `0..255`, unsigned 64-bit decimal immediates, labels, comments, and the
closed `db` string encoding. It emits no host container. Current local build
scripts may stamp an emitted tape into a native Alpha seed because the seeds do
not yet load an external tape directly.

- `beta_assembler_bytecode.tape` — direct Alpha implementation and canonical
  cold-start compiler artifact.
- `assembler.beta` — readable self-host source.
- `artifact_env.sh` — materializes the direct tape in the selected Alpha VM.
- `build.sh` — `./build.sh PROGRAM.beta` produces a stamped local executable.
- `selfhost.sh` — reconstructs the exact direct tape byte-for-byte.
- `asm_ref.py` / `asm-diamond.sh` — temporary independent executable relation
  and differential regression.
- `examples/` — small Beta assembly cases.

## Retention inventory

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `beta_assembler_bytecode.tape` | Sole directly retained Alpha program implementing Beta assembly. | Replace atomically with its exact checked relation and reconstruction. |
| `assembler.beta` | Readable self-host reconstruction source. | Delete only if another equally direct reconstruction replaces it. |
| `artifact_env.sh` | Materialize the canonical raw tape through the selected Alpha VM without retaining a second native binary. | Delete when Alpha loads arbitrary tapes directly. |
| `build.sh`, `selfhost.sh` | Disposable stamping and exact reconstruction. | Delete stamping after raw-tape loading; retain reconstruction until stronger checked evidence subsumes it. |
| `asm_ref.py`, `asm-diamond.sh` | Independent diagnostic implementation. | Delete when the checked assembly relation subsumes its failure detection. |
| `register-label-regression.sh` | Closed lexical, operand, and width discriminator. | Delete when generated checked vectors cover every retained boundary. |
| `examples/` | Small executable encoding controls. | Delete cases only when an equally direct generated control subsumes them. |
| `echo.beta`, `factorial.beta`, `fib.beta`, `gcd.beta`, `multiply.beta` | Closed example set for I/O, recursion, calls, and arithmetic encoding. | Delete an example only when an equally direct generated control subsumes it. |
| `.gitignore` | Keeps disposable stamped programs and tapes out of source ownership. | Delete when local build output moves outside this owner. |

The assembler is a compiler rung, not a second native implementation. Only the
Alpha VM is hand-authored per platform.
