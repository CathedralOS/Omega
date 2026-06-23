# The Beta language (v0 — DRAFT, iterating)

> The pleasant structured layer above raw Alpha assembly: the language you write a
> compiler in, so nothing above it is ever hand-assembled again. v0 surface; will
> be refined toward an elegant shape, then implemented.
>
> **Naming caveat:** this "promote beta from assembler to language" choice is one
> coherent reading; the existing imperative `compiler/gamma/` is most of this
> machinery already (lex/parse/codegen), minus procedures + arrays. Whether the
> pleasant rung keeps the name *beta* or folds the existing gamma in is a separate,
> reversible naming decision — the *surface* below is the same either way.

## What it is

A small, monomorphic, structured systems language — roughly Oberon-0 / tiny C: one
scalar type (`i64`), raw memory access, procedures with parameters and locals
(lowered to the [calling convention](CALLING_CONVENTION.md)), and `if`/`while`.
That is exactly enough to write a lexer, parser, symbol table, and code emitter —
the assembler proves the floor (it does all of this in raw assembly; Beta just
makes it pleasant). No types beyond `i64`, no generics, no proofs — those are
higher rungs.

## Grammar (v0)

```
program    := proc*
proc       := 'proc' IDENT '(' params? ')' block
params     := IDENT (',' IDENT)*
block      := '{' statement* '}'
statement  := 'let' IDENT '=' expr            ; declare + init a local
            | IDENT '=' expr                  ; assign a local
            | store                           ; write memory
            | 'if' expr block ('else' block)?
            | 'while' expr block
            | 'return' expr
            | call                            ; call for effect (result discarded)
expr       := sum (cmpop sum)?                ; a comparison yields 0 / 1
sum        := term (('+' | '-') term)*
term       := factor (('*' | '/' | '%') factor)*
factor     := INT | IDENT | call | load | '(' expr ')'
call       := IDENT '(' (expr (',' expr)*)? ')'
load       := 'byte' '[' expr ']'   |   'word' '[' expr ']'
store      := ('byte' | 'word') '[' expr ']' '=' expr
cmpop      := '<' | '>' | '==' | '<=' | '>=' | '!='
```

`;`-to-end-of-line comments. `read_byte()` / `write_byte(x)` are built-in calls
(the only host boundary, straight to Alpha `read`/`write`).

## Lowering (every construct maps to what we already have)

| Construct | Lowers to |
| --- | --- |
| `proc f(a, b) { ... }` | a label `f:`; params arrive in `r0`,`r1`; prologue `sub r15, framesize`; epilogue `add r15, framesize; ret` |
| `let x = e` / `x = e` | a frame slot at `[r15 + off]`; evaluate `e`, `store` into the slot |
| reading `x` | `load` from its frame slot |
| `f(args...)` | evaluate args into `r0..r3` (spilling caller-saved live values to the frame first); `call f`; result in `r0` |
| `expr` | the gamma-style stack-machine codegen for `+ - * / %` and comparisons (already written, in assembly, in `gamma.alpha`) |
| `if` / `while` | `jz`/`jmp` over blocks (already in gamma) |
| `return e` | evaluate `e` into `r0`; epilogue; `ret` |
| `byte[e]` / `word[e]` | `loadb` / `load`; the store forms → `storeb` / `store` |

The only genuinely *new* machinery over today's gamma is **procedures with
frames** (the calling convention, now proven) and **explicit memory access**
(`byte[]` / `word[]`). Everything else is gamma's existing lexer/parser/expression
codegen, reused.

## Build order (next iterations)

1. ✅ Calling convention — designed + proven on the seed.
2. Procedures + locals + `return` over the convention (the new codegen).
3. Explicit memory (`byte[]`/`word[]`) — gives arrays/buffers without records.
4. Named identifiers + a symbol table (beyond gamma's fixed `a`–`j`).
5. Self-check: write a non-trivial program in Beta (e.g. re-express the assembler's
   core loop) to confirm it is actually pleasant to write a compiler in.
6. Then: rewrite gamma **in Beta**, never again in assembly.

## Open questions (to iterate)

- Records/structs, or stay raw-memory + offsets (as the assembler does) until a
  higher rung? (Lean: raw memory for v0; records are sugar later.)
- Multiple return values / out-params, or single `r0` return only? (Lean: single.)
- `i64` only, or add `u8`/byte values as a type rather than via `byte[]`? (Lean:
  `i64` + `byte[]`/`word[]` access; no byte *type* yet.)
- The naming/rung reconciliation with the existing imperative gamma (above).
