# The Gamma language

> The small structured systems layer above Beta assembly. The canonical
> compiler is implemented in Beta.
> The rung name and place in `Alpha → Beta → Gamma → Delta → Epsilon` are fixed by
> bootstrap decision D6.
> Runtime meaning is fixed separately by [`SEMANTICS.md`](SEMANTICS.md).

## What it is

A small, monomorphic systems language — one scalar type (`i64`), raw memory access,
procedures with parameters and locals (lowered to the
[calling convention](CALLING_CONVENTION.md)), and **CFG/Omega-style control flow**:
`state` basic blocks linked by guarded `to … when …` transitions, no `if`/`while`.
Control flow is a state graph (the shape the higher Omega rung uses), not structured
statements — a proc falls into its first state and into the next unless it transitions
or returns; a loop is a state that transitions back to itself.
That is exactly enough to write a lexer, parser, symbol table, and code emitter —
the assembler proves the floor (it does all of this in raw assembly; Gamma just
makes it pleasant). No types beyond `i64`, no generics, no proofs — those are
higher rungs.

## Grammar (v1)

```
program    := proc*
proc       := 'proc' IDENT '(' params? ')' block
params     := IDENT (',' IDENT)*
block      := '{' ordinary* state* '}'
state      := 'state' IDENT block              ; recursively authored, flat CFG label
ordinary   := 'let' IDENT '=' expr             ; declare + init a local
            | IDENT '=' expr                  ; assign a local
            | store                           ; write memory
            | 'to' IDENT ('when' expr)?       ; a transition: jump, or guarded jump
            | 'return' expr
            | call                            ; call for effect (result discarded)
            | 'emit' '(' STRING ')'           ; fixed byte output only
expr       := sum (cmpop sum)?                ; a comparison yields 0 / 1
sum        := term (('+' | '-') term)*
term       := factor (('*' | '/' | '%') factor)*
factor     := INT | CHAR | IDENT | call | load | '(' expr ')'
call       := IDENT '(' (expr (',' expr)*)? ')'
load       := 'byte' '[' expr ']'   |   'word' '[' expr ']'
store      := ('byte' | 'word') '[' expr ']' '=' expr
cmpop      := '<' | '>' | '==' | '<=' | '>=' | '!='
CHAR       := "'" (char | '\\' ('n'|'t'|'r'|'0'|'\\'|"'")) "'"   ; the byte value
INT        := decimal digits whose mathematical value is in 0..2^64-1
```

## Lexical form

Gamma source is a byte stream in the bootstrap textual-ASCII envelope. The only
admitted source bytes are horizontal tab (`0x09`), line feed (`0x0A`), carriage
return (`0x0D`), and printable ASCII (`0x20..0x7E`). NUL, DEL, bytes above
`0x7F`, and every other control byte reject before tokenization at their exact
byte offset. There is no source decoding, BOM, Unicode normalization,
host-locale rule, or Unicode character classification.

Space, tab, CR, and LF are the complete whitespace set. `;` begins a comment
through the next CR, LF, or source end. Identifiers match
`[A-Za-z_][A-Za-z0-9_]*`; decimal digits are exactly `0..9`. A direct character
literal byte is printable ASCII except single quote and backslash, which use
the closed escapes in the grammar. A direct `emit` string byte is printable
ASCII except double quote and backslash; its escapes are exactly
`\n \t \r \0 \\ \"`.

These are source rules, not value restrictions. `write_byte(x)` can emit any
low byte of `x`, and escapes can produce admitted control data without placing
the corresponding control byte raw in source.

**GAMMA-FLATTENED-CFG-INITIALIZATION.** Every procedure body and state body has
the same recursive authored shape: an ordinary-statement prefix followed by
zero or more state declarations. Once a state declaration begins in one block,
no loose ordinary statement may follow it in that block. Within the ordinary
prefix, `return` and an unconditional `to` must be the final ordinary statement;
following state declarations remain legal because they declare separately
targetable blocks.

Nesting organizes source but does not create hierarchical runtime control or
scope. State declarations flatten to procedure-wide labels in exact depth-first
lexical order: emit a state's label, visit its ordinary prefix, recursively
visit its child states, then continue with the next state after that subtree.
An unterminated final child therefore falls through to the next outer sibling.
All state names are unique and targetable throughout their procedure; braces do
not qualify a state name or restrict a `to` edge.

Locals likewise occupy procedure-wide frame slots rather than block scopes. A
`let` declaration becomes visible from its position in the flattened lexical
order through the remainder of the procedure. Its initializer establishes the
slot only after the expression succeeds. A later assignment to a resolved slot
also establishes it after evaluating its right-hand side; reads require
every-path establishment under the rules in `SEMANTICS.md`.

`;`-to-end-of-line comments. `read_byte()` / `write_byte(x)` are built-in calls
(the only host boundary, straight to Alpha `read`/`write`); a call may also stand
alone as a statement (`f(x)`), evaluated for effect with its result discarded.
A char literal `'a'` is just its byte value (an `INT`), so text-processing code
reads in characters instead of magic numbers (`peek() - '0'`, `c == '('`).
Leading zeros are permitted in an `INT`; the range check is on its mathematical
value and occurs before any wrapping machine accumulation.

`emit("text")` is the one place a string literal is allowed: it writes the bytes
to stdout (lowering to a `write` per byte). There is **no string type** — it is a
write-only convenience so a compiler written in Gamma can emit fixed output (e.g.
assembly mnemonics) without spelling every byte. `"..."` escapes are exactly
`\n \t \r \0 \\ \"`.

## Lowering (every construct maps to what we already have)

| Construct | Lowers to |
| --- | --- |
| `proc f(a, b) { ... }` | a label `f:`; params arrive in `r0`,`r1`; an `r14` frame-pointer prologue allocates below `r15`; the epilogue restores both |
| `let x = e` / `x = e` | a frame slot at `[r14 - off]`; evaluate `e`, `store` into the slot |
| reading `x` | `load` from its frame slot |
| `f(args...)` | evaluate args into `r0..r3` (spilling caller-saved live values to the frame first); `call f`; result in `r0` |
| `expr` | stack-based evaluation for `+ - * / %` and comparisons |
| `state S { ... }` | a procedure-wide label `<proc>__S:` in depth-first lexical order; falls through to the next flattened state |
| `to S` / `to S when e` | `jmp <proc>__S` / evaluate `e`, `jz` past a `jmp` to it |
| `return e` | evaluate `e` into `r0`; epilogue; `ret` |
| `byte[e]` / `word[e]` | checked logical raw-memory address, physical-region bias, then `loadb` / `load`; stores analogously |

## Deliberate limits

- One scalar type: `i64`.
- At most four register arguments and one `r0` return value.
- Raw `byte[]` and `word[]` memory rather than records or a safe object model.
- No algebraic data, pattern matching, ownership, effects, generics, or proofs.
- String literals exist only inside `emit`; there is no string value type.

These are rung boundaries, not unfinished Delta or Delta features. The
canonical Beta-written compiler is
[`compiler/gamma_compiler.beta`](compiler/gamma_compiler.beta).
The former self-host and duplicate historical-compiler surface suite were
deleted once the direct compiler and focused gate subsumed their useful role.
