# Alpha-written Beta cold start

This directory contains the authoritative cold start for `bc.beta`: a Beta
compiler written in Alpha and assembled/executed only through the audited Alpha
seed and Alpha-written assembler. It accepts the complete pinned `bc.beta`
surface and replaces the Rust producer in the canonical construction.

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
contains only 913 literal payload bytes, so this direct lowering remains well
inside the checked output extent.

The compiler reads at most 1,048,576 source bytes into a checked fixed extent,
bounds expression/call nesting at 64, procedure count at 128, recorded calls at
512, frame slots per procedure at 64, and states/transitions at 64 per procedure
and 512 globally. It preflights emitted assembly against the Alpha assembler's
1 MiB source region. The first parse records frozen procedure, call, state, and
transition metadata; resolves calls and procedure-scoped edges after EOF; and
reserves exact output without publishing it. The second parse checks every
frozen record in source order and streams the assembly. Malformed and exhausted
inputs therefore halt nonzero with an empty output stream. Generated frames
preserve `r14`/`r15`, four live argument registers, full epilogues, and
precedence-correct lowering. Generated `$L…` and `$S…$…` labels use bytes that
Beta identifiers cannot spell, preventing collisions with source names.

This now accepts the exact pinned `bc.beta` profile and closes its external-
producer cold-start dependency. [`full-source.sh`](full-source.sh) builds `bc.beta` through the
Alpha-written compiler, advances to the self-hosted fixed point, reconstructs
[`../artifacts/bc.tape`](../artifacts/README.md) byte-for-byte, and runs the whole
Beta corpus through that persisted artifact. This proves construction lineage,
reproducibility, and retained behavior. The separate ROOT gate under
`source/beta/compiler/validation/` now closes lower-rooted source-to-artifact
refinement for the exact persisted tape and `B_bc1` profile.

## Full-source target profile

The target is pinned to the current `bc.beta` source, SHA-256
`f844d33e29814f1280bbeee2bf599db2bded2fb9469a7f1bfc870fac522c326d`:
32,064 bytes, 5,810 tokens, 70 procedures, at most two parameters or call
arguments, five frame slots, seventeen states and twenty-four transitions per
procedure, expression depth five, and identifier length eighteen. Across the
source it has 285 states, 291 transitions, and 180 comparison nodes. It uses
every arithmetic and comparison operator, byte/word memory, calls, CFG
transitions, byte I/O, and fixed-string emission. These measurements define
implementation capacities; they do not broaden Beta's language meaning.

Run the focused gate with:

```sh
sh source/beta/compiler/cold-start/test.sh
```
