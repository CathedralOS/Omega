# Canonical Beta compiler construction

This directory owns the one-step construction and focused language tests for
[`../beta_compiler.alpha`](../beta_compiler.alpha). The Alpha-written assembler
turns that source directly into the persisted compiler tape; no Beta self-host
or textual-output assembler stage follows it.

## Complete Beta surface

[`../beta_compiler.alpha`](../beta_compiler.alpha) currently accepts exactly:

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
limit. Decimal literals cover the complete `0..2^64-1` Word range; leading zeros
are permitted and overflow is rejected before wrapping. Character literals cover printable single bytes and `\n`, `\t`, `\r`,
`\0`, `\\`, and `\'` escapes. A zero-parameter `main` must exist. Calls may be
forward, backward, or nested; parameters and arguments are limited to four, and
locals are function-scoped frame slots under Beta's calling convention. Keywords
and the intrinsic names `read_byte` and `write_byte` are reserved from procedure,
local, parameter, and state declarations.
Comparisons are signed except full-width equality and materialize exactly zero
or one. Procedure-scoped `state` labels fall through in source order;
unconditional `to` jumps, while `to … when expr` jumps only for nonzero guards.
Byte/word memory is a 32 MiB zeroed logical region biased to physical Alpha
addresses `2097152..35651583`; generated signed-negative and exclusive-upper
guards run before the bias and every load/store. It is therefore disjoint from
the tape and generated data-stack region. Every generated frame or expression
reservation checks the data-stack floor before its next access; the mandatory
frame word also bounds Alpha's hidden return stack. Exhaustion halts with status
250 inside the current procedure. `read_byte()` and
`write_byte(x)` are the sole runtime I/O intrinsics. `emit("…")` decodes Beta's
six string escapes and emits one Alpha `write` per byte inside the checked
output extent.

The compiler reads at most 1,048,576 source bytes into a checked fixed extent,
bounds expression/call nesting at 64, procedure count at 128, recorded calls at
1,024, frame slots per procedure at 64, states at 128 per procedure, transitions
at 256 per procedure, and each table at 1,024 globally. It preflights the
actual 262,140-byte Alpha payload extent. The first parse records frozen
procedure, call, state, and transition metadata; resolves calls and
procedure-scoped edges to numeric identities after EOF; and reserves exact
instruction widths without publishing anything. The second parse checks every
frozen record in source order while encoding into a private tape buffer.
Bounded procedure/state/internal-label PC tables and a bounded fixup table own
all forward addresses. Only after every fixup has been resolved and the replay
length has matched the reservation does the compiler publish the completed
tape to stdout. Malformed, exhausted, or internally inconsistent inputs
therefore halt nonzero with an empty output stream. Generated frames
initialize the reserved word-size register `r13` to eight, preserve `r14`/`r15`,
carry four live argument registers, return zero on final fallthrough, and retain
explicit return values through full epilogues. Generated `$L…` and `$S…$…`
labels use bytes that Beta identifiers cannot spell, preventing collisions with
source names.

This accepts arbitrary programs in the bounded surface above.
[`rebuild-artifact.sh`](rebuild-artifact.sh) assembles the canonical Alpha source
directly. Its `--check` mode reconstructs
[`../beta_compiler_bytecode.tape`](../beta_compiler_bytecode.tape) byte-for-byte without changing
the repository; its default mode deliberately installs that reconstruction.
The focused [`test.sh`](test.sh) exercises the compiler's accepted and
rejected Beta surface, but that regression suite is not a compiler-lattice
edge. The direct emitter was migration-checked byte-for-byte against the former
textual-output-plus-Alpha-assembler route, after which that obsolete oracle was
removed. Its stored-word encoder subsequently corrected the old assembler's
high-bit `u64` quotient bug, so that intentional semantic repair supersedes
blanket byte identity with the historical route. The adjacent validation
directory now retains only a general artifact-structure check and a bounded
symbolic differential. The former self-hosted
compiler and its exact admission forest were deleted because none of their
source/PC/count-specific propositions transferred to the promoted Alpha source.

Run the focused gate with:

```sh
sh source/beta/compiler/cold-start/test.sh
```

Recheck the canonical construction with:

```sh
sh source/beta/compiler/cold-start/rebuild-artifact.sh --check
```
