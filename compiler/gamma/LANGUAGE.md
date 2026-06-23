# The gamma language

A small structured language that compiles to alpha assembly. Integers only (signed
64-bit; `print` and `read` are decimal, non-negative). Compiled by `gamma_x64_windows.exe`
to assembly, which `beta` lowers to a tape stamped into the alpha seed → a standalone exe.

## Grammar

```
program   := statement*
statement := var '=' expr
           | 'if' expr '{' statement* '}' [ 'else' '{' statement* '}' ]
           | 'while' expr '{' statement* '}'
           | 'print' expr            ; writes the decimal value + newline to stdout
           | 'read' var              ; reads a decimal from stdin into var
expr       := sum (cmpop sum)?       ; a comparison yields 0 or 1
cmpop      := '<' | '>' | '==' | '<=' | '>=' | '!='
sum        := term (('+' | '-') term)*
term       := factor (('*' | '/' | '%') factor)*
factor     := number | var | '(' expr ')'
var        := a single letter a..j           ; held in registers r6..r15
```

- `# …` to end of line is a comment.
- `if`/`while` run/loop while the condition is **nonzero**; comparisons produce 0/1.
- `* / %` bind tighter than `+ -`; comparisons are looser than both; `( )` override.
- The program **exits with the value of the last variable assigned** (low byte as the
  process exit code). Use `print` for full values.

## Examples (in `examples/`)

```
# primes up to n
read a
b = 2
while b <= a {
  c = 2
  d = 1
  while c < b {
    e = b % c
    if e == 0 { d = 0 }
    c = c + 1
  }
  if d == 1 { print b }
  b = b + 1
}
```

`gcd.gam` (Euclid), `fib.gam` (iterative Fibonacci), `primes.gam`, `squares.gam`,
`countdown.gam`, … — build any with `./build.sh examples/NAME.gam` then run
`./build/NAME.exe` (pipe input for the ones that `read`).

## Limits (current)

- 10 variables (`a`–`j`). Lifting this needs named identifiers + a symbol table.
- No functions, arrays, or string output yet.
- These are the features needed before gamma can be rewritten **in gamma** (self-hosting).
