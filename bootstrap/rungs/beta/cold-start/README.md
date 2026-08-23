# Alpha-written Beta cold start

This directory grows the authoritative cold start for `bc.beta`: a Beta
compiler written in Alpha and assembled/executed only through the audited Alpha
seed and Alpha-written assembler. It replaces the Rust producer one bounded
language slice at a time. It is not a second compiler maintained for DDC.

## Slice A

[`bc-alpha.alpha`](bc-alpha.alpha) currently accepts exactly:

```text
program := "proc" IDENT "(" ")" "{" "return" expr "}"
expr    := term (("+" | "-") term)*
term    := factor (("*" | "/" | "%") factor)*
factor  := DECIMAL | CHAR | "(" expr ")"
```

Whitespace and `;` line comments may occur between tokens. Identifiers use
ASCII letters, digits after the first character, and `_`. Decimal literals are
nonnegative and limited to nine digits for this slice. Character literals cover
printable single bytes and `\n`, `\t`, `\r`, `\0`, `\\`, and `\'` escapes.
The sole procedure must be named `main`, preserving Beta's real entry rule;
multiple names become observable when procedures and calls land in the next
slice.

The compiler reads at most 1,048,576 source bytes into a checked fixed extent,
bounds parenthesis nesting at 64, and preflights the emitted assembly against the
Alpha assembler's 1 MiB source region. It runs the same parser twice: validation
with publication disabled, then emission. Malformed and exhausted inputs
therefore halt nonzero with an empty output stream. Valid input produces ordinary
Alpha assembly with the established Beta data-stack convention and
precedence-preserving expression lowering.

This is not yet the exact `bc.beta` profile and does not close the cold-start
edge. The monotonic path is identifiers/procedures and constant expressions,
then locals and calls, state graphs, memory and byte I/O, and finally the exact
bounded capacities exercised by `bc.beta`. Persisting and adopting a
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
