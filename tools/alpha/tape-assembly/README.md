# Alpha Tape Assembly

Alpha Tape Assembly is off-chain construction and inspection tooling for raw
Alpha tapes. It gives Alpha instructions human-readable mnemonics, labels,
decimal operands, and data spelling without becoming a bootstrap language rung.

[`LANGUAGE.md`](LANGUAGE.md) defines the exact deterministic relation from one
Alpha Tape Assembly source byte sequence to one raw Alpha `.tape`. The compiler is the
platform-independent [`compiler/alpha_tape_assembler_bytecode.tape`](compiler/alpha_tape_assembler_bytecode.tape),
an exact retained Alpha tool. Its readable
[`compiler/assembler.alphaasm`](compiler/assembler.alphaasm) source must
reconstruct that tape byte-for-byte.

```text
audited native Alpha VM
  + alpha_tape_assembler_bytecode.tape
  + program.alphaasm
    -> program.tape
```

Callers materialize that tape in the selected Alpha VM only for the duration of
an invocation. No second platform-native assembler binary is retained.

This tool may help author or inspect Alpha programs, including the Beta
evaluator, but no compiler edge depends on an Alpha Tape Assembly judgment.
Successful assembly yields Alpha tape whose execution is governed solely by
[`source/alpha/SEMANTICS.md`](../../../source/alpha/SEMANTICS.md).

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Exact Alpha-Tape-Assembly-text-to-Alpha-tape relation. | Replace only with a versioned tool encoding and synchronized assembler. |
| `compiler/` | Direct assembler tape and readable reconstruction source. | Delete when a smaller equally auditable tool subsumes its uses. |
| `compiler/assembler.alphaasm` | Readable reconstruction source for the Alpha Tape assembler. | Delete only if another exact reconstruction replaces it. |
| `compiler/alpha_tape_assembler_bytecode.tape` | Direct portable Alpha implementation of Alpha Tape Assembly. | Replace atomically with its source, reconstruction, and checked relation. |

Assembler tests and examples live under `tests/alpha/tape-assembly/compiler/`; host
materialization lives under `tools/alpha/tape-assembly/`.
