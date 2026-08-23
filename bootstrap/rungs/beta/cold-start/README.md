# Alpha-written Beta cold start

This directory grows the authoritative cold start for `bc.beta`: a Beta
compiler written in Alpha and assembled/executed only through the audited Alpha
seed and Alpha-written assembler. It replaces the Rust producer one bounded
language slice at a time. It is not a second compiler maintained for DDC.

## Slices A–B

[`bc-alpha.alpha`](bc-alpha.alpha) currently accepts exactly:

```text
program   := proc+
proc      := "proc" IDENT "(" params? ")" "{" statement* "}"
params    := IDENT ("," IDENT)*
statement := "let" IDENT "=" expr | IDENT "=" expr | "return" expr
expr      := term (("+" | "-") term)*
term      := factor (("*" | "/" | "%") factor)*
factor    := DECIMAL | CHAR | IDENT | IDENT "(" args? ")" | "(" expr ")"
args      := expr ("," expr)*
```

Whitespace and `;` line comments may occur between tokens. Identifiers use
ASCII letters, digits after the first character, and `_`, with a checked 64-byte
limit. Decimal literals are nonnegative and limited to nine digits for these
slices. Character literals cover printable single bytes and `\n`, `\t`, `\r`,
`\0`, `\\`, and `\'` escapes. A zero-parameter `main` must exist. Calls may be
forward, backward, or nested; parameters and arguments are limited to four, and
locals are function-scoped frame slots under Beta's calling convention.

The compiler reads at most 1,048,576 source bytes into a checked fixed extent,
bounds expression/call nesting at 64, procedure count at 128, recorded calls at
512, and frame slots per procedure at 64. It preflights emitted assembly against
the Alpha assembler's 1 MiB source region. The first parse records frozen
procedure signatures and final frame sizes, resolves every call after EOF, and
reserves exact output without publishing it. The second parse checks that
metadata and streams the assembly. Malformed and exhausted inputs therefore halt
nonzero with an empty output stream. Generated frames preserve `r14`/`r15`, four
live argument registers, full epilogues, and precedence-correct lowering.

This is not yet the exact `bc.beta` profile and does not close the cold-start
edge. The monotonic path now continues through state graphs and comparisons,
memory and byte I/O, and finally fixed-string emission plus the remaining exact
capacities exercised by `bc.beta`. Persisting and adopting a
lattice-built `bc` artifact waits until that complete profile passes the whole
Beta corpus and self-build gates.

## Full-source target profile

The target is pinned to the current `bc.beta` source, SHA-256
`5f2113055a46da6fe1b988bd6c269acc73f19f2ff3d8629ef5a25b7ce276c0da`:
30,307 bytes, 5,479 tokens, 70 procedures, at most two parameters or call
arguments, five frame slots, seventeen states per procedure, expression depth
five, and identifier length eighteen. It uses every arithmetic and comparison
operator, byte/word memory, calls, CFG transitions, byte I/O, and fixed-string
emission. These measurements define implementation capacities; they do not
broaden Beta's language meaning.

Run the focused gate with:

```sh
sh bootstrap/rungs/beta/cold-start/test.sh
```
