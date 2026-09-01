# Beta rung

Beta is the first textual language above the audited Alpha VM. It is a compact,
NASM-like spelling of Beta instructions with labels, decimal operands,
comments, and exact byte data.

```text
assembler.beta --(beta_assembler_bytecode.tape on Alpha)--> Alpha tape
```

The direct implementation is the platform-independent
`source/beta/compiler/beta_assembler_bytecode.tape`. The readable
`assembler.beta` reconstructs that tape byte-for-byte. Callers materialize the
raw tape in the selected Alpha VM for one invocation; no separate native
assembler binary is retained per platform.

Beta has no runtime meaning beyond Alpha. Successful assembly produces raw
Alpha tape, and `source/alpha/SEMANTICS.md` alone defines execution of that
tape. Beta's contract is the deterministic text-to-tape relation in
`source/beta/LANGUAGE.md`.

The canonical customer is the Beta-written Gamma compiler. Other small
trust-floor tools may also be authored in Beta when doing so is simpler than
constructing raw tape bytes by hand.
