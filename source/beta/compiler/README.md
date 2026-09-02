# Beta compiler

This directory owns the compiler from Beta text to raw Alpha tape.
The authoritative grammar and encoding are fixed by
[`../LANGUAGE.md`](../LANGUAGE.md).

The admitted cold-start implementation is
[`beta_compiler_bytecode.tape`](beta_compiler_bytecode.tape), a 2,706-byte
platform-independent Alpha program at the first compiler edge. The readable
[`beta_compiler.beta`](beta_compiler.beta) source is the same assembler expressed in
Beta; `tests/beta/compiler/reconstruction.sh`
requires it to reproduce the direct tape byte-for-byte.

The compiler accepts mnemonic opcodes, complete-token lowercase hexadecimal
registers `rH`/`rHH`, `0x`-prefixed lowercase hexadecimal 64-bit words,
lowercase labels, comments, and the closed `db` string encoding. Its two passes
share one scanner; mnemonic rows carry their NUL-terminated operand-width lists;
and bounded source pointers plus contiguous label rows are retained in a
documented high-register ABI. It emits no host container. Current local build
scripts may stamp an emitted tape into a native Alpha seed because the seeds do
not yet load an external tape directly.

The minimized implementation keeps the audit-friendly two-pass algorithm. It
uses one mode-driven scanner, validates source bytes while reading, retains
absolute bounded source pointers, stores labels in contiguous exact-name rows,
shares one hexadecimal parser and one exact label lookup, and drives operands
from visible NUL-terminated width lists beside each mnemonic. Alpha's specified
zero-initialized register file supplies named persistent state and constants.
One-pass backpatch chains, hashes, compressed labels, comma removal, and new
Alpha opcodes were rejected because their additional trust cost outweighed the
remaining byte savings.

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
