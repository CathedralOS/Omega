# Beta assembly language

Beta is the first textual rung above Alpha. Alpha owns only the raw portable
instruction tape and the native VM that executes it; Beta gives those
instructions human-readable mnemonics, labels, decimal operands, and data
spelling.

[`LANGUAGE.md`](LANGUAGE.md) defines the exact deterministic relation from one
Beta source byte sequence to one raw Alpha `.tape`. The compiler is the
platform-independent [`compiler/beta_assembler_bytecode.tape`](compiler/beta_assembler_bytecode.tape),
the one directly retained Alpha program at the cold-start boundary. Its
readable [`compiler/assembler.beta`](compiler/assembler.beta) source must
self-host back to that exact tape.

```text
audited native Alpha VM
  + beta_assembler_bytecode.tape
  + program.beta
    -> program.tape
```

Callers materialize that tape in the selected Alpha VM only for the duration of
an invocation. No second platform-native assembler binary is retained.

Beta exists to author the Gamma compiler and other trust-floor Alpha programs.
It has no independent runtime semantics: successful assembly yields Alpha tape,
whose execution is governed solely by [`../alpha/SEMANTICS.md`](../alpha/SEMANTICS.md).

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Exact Beta-text-to-Alpha-tape relation. | Replace only with a versioned Beta encoding and synchronized assembler. |
| `compiler/` | Direct assembler tape and readable reconstruction source. | Delete only when an equally direct Beta implementation replaces the owner. |
| `compiler/assembler.beta` | Readable self-host source for the Beta assembler. | Delete only if another exact reconstruction of the direct assembler tape replaces it. |
| `compiler/beta_assembler_bytecode.tape` | Direct portable Alpha implementation of Beta assembly. | Replace atomically with its source, reconstruction, and checked relation. |

Assembler tests and examples live under `tests/beta/compiler/`; host
materialization lives under `tools/bootstrap/beta/`.
