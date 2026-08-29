# Alpha-written Beta cold start

This directory contains the existing Alpha-written Beta compiler candidate. It
is assembled and executed only through the audited Alpha seed and Alpha-written
assembler. Today it accepts the complete pinned `bc.beta` surface and feeds a
self-hosted fixed point. The canonical edge instead promotes and generalizes
this Alpha implementation so it directly owns the persisted Beta compiler tape;
the fixed point then becomes diagnostic.

## Complete Beta surface

[`bc-alpha.alpha`](bc-alpha.alpha) currently accepts exactly:

```text
program   := proc+
proc      := "proc" IDENT "(" params? ")" "{" statement* "}"
params    := IDENT ("," IDENT)*
statement := "let" IDENT "=" expr | IDENT "=" expr | "return" expr
           | "state" IDENT "{" statement* "}"
           | "to" IDENT ("when" expr)?
           | ("byte" | "word") "[" expr "]" "=" expr
           | IDENT "(" args? ")" | "emit" "(" STRING ")"
expr      := sum (("<" | ">" | "==" | "!=" | "<=" | ">=") sum)?
sum       := term (("+" | "-") term)*
term      := factor (("*" | "/" | "%") factor)*
factor    := DECIMAL | CHAR | IDENT | IDENT "(" args? ")"
           | ("byte" | "word") "[" expr "]" | "(" expr ")"
args      := expr ("," expr)*
```

Whitespace and `;` line comments may occur between tokens. Identifiers use
ASCII letters, digits after the first character, and `_`, with a checked 64-byte
limit. Decimal literals are nonnegative and limited to nine digits for these
slices. Character literals cover printable single bytes and `\n`, `\t`, `\r`,
`\0`, `\\`, and `\'` escapes. A zero-parameter `main` must exist. Calls may be
forward, backward, or nested; parameters and arguments are limited to four, and
locals are function-scoped frame slots under Beta's calling convention.
Comparisons are signed except full-width equality and materialize exactly zero
or one. Procedure-scoped `state` labels fall through in source order;
unconditional `to` jumps, while `to … when expr` jumps only for nonzero guards.
Byte/word memory lowers directly to Alpha loads and stores. `read_byte()` and
`write_byte(x)` are the sole runtime I/O intrinsics. `emit("…")` decodes Beta's
six string escapes and emits one Alpha `write` per byte; the pinned `bc.beta`
contains only 791 literal payload bytes, so this direct lowering remains well
inside the checked output extent.

The compiler reads at most 1,048,576 source bytes into a checked fixed extent,
bounds expression/call nesting at 64, procedure count at 128, recorded calls at
1,024, frame slots per procedure at 64, states at 128 per procedure, transitions
at 256 per procedure, and each table at 1,024 globally. It preflights emitted assembly against the Alpha assembler's
1 MiB source region. The first parse records frozen procedure, call, state, and
transition metadata; resolves calls and procedure-scoped edges after EOF; and
reserves exact output without publishing it. The second parse checks every
frozen record in source order and streams the assembly. Malformed and exhausted
inputs therefore halt nonzero with an empty output stream. Generated frames
preserve `r14`/`r15`, four live argument registers, full epilogues, and
precedence-correct lowering. Generated `$L…` and `$S…$…` labels use bytes that
Beta identifiers cannot spell, preventing collisions with source names.

This accepts the exact pinned `bc.beta` profile and closes its external-producer
dependency for the historical construction. [`rebuild-artifact.sh`](rebuild-artifact.sh)
builds `bc.beta` through the Alpha-written compiler and advances to the
self-hosted fixed point. Its `--check` mode reconstructs
[`../artifacts/beta_compiler_bytecode.tape`](../artifacts/README.md) byte-for-byte without changing
the repository; its default mode deliberately installs that reconstruction.
The focused [`test.sh`](test.sh) exercises the cold compiler's accepted and
rejected Beta surface, but that regression suite is not a compiler-lattice
edge. The separate ROOT gate under `source/beta/compiler/validation/`
reconstructs the exact persisted `bc.beta` tape and `B_bc1` profile. Its general
machinery must be adapted to the promoted Alpha source; it does not close the
canonical Beta edge as written.

## Full-source target profile

The target is pinned to the current `bc.beta` source, SHA-256
`b6ad15ed9cc540a628b83c671bd8c6629770056a641d72d885e41354a8b06c4c`:
32,605 bytes. Its measured surface remains inside every adjacent-boundary-tested
cold-compiler capacity. It uses
every arithmetic and comparison operator, byte/word memory, calls, CFG
transitions, byte I/O, and fixed-string emission. These measurements define
implementation capacities; they do not broaden Beta's language meaning.

Run the focused gate with:

```sh
sh source/beta/compiler/cold-start/test.sh
```

Recheck the current migration construction with:

```sh
sh source/beta/compiler/cold-start/rebuild-artifact.sh --check
```
