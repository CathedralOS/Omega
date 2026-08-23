# The Beta language

> The small structured systems layer above Alpha assembly. `bc.beta` implements
> this surface and self-hosts; the rung name and place in
> `Alpha → Beta → Gamma → Delta` are fixed by bootstrap decision D6.
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
            | 'state' IDENT block             ; a CFG basic block (a label + its body)
            | 'to' IDENT ('when' expr)?       ; a transition: jump, or guarded jump
            | 'return' expr
            | call                            ; call for effect (result discarded)
expr       := sum (cmpop sum)?                ; a comparison yields 0 / 1
sum        := term (('+' | '-') term)*
term       := factor (('*' | '/' | '%') factor)*
factor     := INT | CHAR | IDENT | call | load | '(' expr ')'
call       := IDENT '(' (expr (',' expr)*)? ')'
load       := 'byte' '[' expr ']'   |   'word' '[' expr ']'
store      := ('byte' | 'word') '[' expr ']' '=' expr
cmpop      := '<' | '>' | '==' | '<=' | '>=' | '!='
CHAR       := "'" (char | '\' ('n'|'t'|'r'|'0'|'\'|"'")) "'"   ; the byte value
```

`;`-to-end-of-line comments. `read_byte()` / `write_byte(x)` are built-in calls
(the only host boundary, straight to Alpha `read`/`write`); a call may also stand
alone as a statement (`f(x)`), evaluated for effect with its result discarded.
A char literal `'a'` is just its byte value (an `INT`), so text-processing code
reads in characters instead of magic numbers (`peek() - '0'`, `c == '('`).

`emit("text")` is the one place a string literal is allowed: it writes the bytes
to stdout (lowering to a `write` per byte). There is **no string type** — it is a
write-only convenience so a compiler written in Beta can emit fixed output (e.g.
assembly mnemonics) without spelling every byte. `"..."` escapes: `\n \t \r \0 \\ \"`.

## Lowering (every construct maps to what we already have)

| Construct | Lowers to |
| --- | --- |
| `proc f(a, b) { ... }` | a label `f:`; params arrive in `r0`,`r1`; an `r14` frame-pointer prologue allocates below `r15`; the epilogue restores both |
| `let x = e` / `x = e` | a frame slot at `[r14 - off]`; evaluate `e`, `store` into the slot |
| reading `x` | `load` from its frame slot |
| `f(args...)` | evaluate args into `r0..r3` (spilling caller-saved live values to the frame first); `call f`; result in `r0` |
| `expr` | stack-based evaluation for `+ - * / %` and comparisons |
| `state S { ... }` | a label `<proc>__S:`; falls through to the next state |
| `to S` / `to S when e` | `jmp <proc>__S` / evaluate `e`, `jz` past a `jmp` to it |
| `return e` | evaluate `e` into `r0`; epilogue; `ret` |
| `byte[e]` / `word[e]` | `loadb` / `load`; the store forms → `storeb` / `store` |

## Deliberate limits

- One scalar type: `i64`.
- At most four register arguments and one `r0` return value.
- Raw `byte[]` and `word[]` memory rather than records or a safe object model.
- No algebraic data, pattern matching, ownership, effects, generics, or proofs.
- String literals exist only inside `emit`; there is no string value type.

These are rung boundaries, not unfinished Gamma or Delta features. The
self-hosting compiler is [`bc.beta`](bc.beta); its
fixed-point and language corpus are gated by `bootstrap/rungs/beta/selfhost.sh`
and `bootstrap/rungs/beta/test.sh`.
