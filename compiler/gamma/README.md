# `compiler/gamma/` — the language above assembly

gamma is the rung above beta: a language a bit higher than raw alpha assembly. Its
compiler is **written in alpha assembly** (`gamma.alp`), assembled by beta into a tape,
and stamped into the alpha seed → `gamma_x64_windows.exe`. gamma reads `.gam` source and
emits assembly; the chain targets one level down at each step:

```
.gam  --[gamma]-->  assembly  --[beta]-->  tape  --[stamp]-->  standalone .exe
```

- `gamma.alp` — gamma's compiler, in alpha assembly.
- `rebuild.sh` — rebuild `gamma_x64_windows.exe` from `gamma.alp` (assemble via beta, stamp).
- `build.sh PROG.gam` → `build/PROG.exe` — run the full chain on a gamma program.
- `examples/` — gamma programs.

```
./rebuild.sh
./build.sh examples/answer.gam && ./build/answer.exe   # exits with the program's value
```

**Status: v11** — variables, arithmetic (with precedence), comparisons, `if`, `while`,
and `print`:
`statement := var '=' expr | 'if' expr '{'…'}' | 'while' expr '{'…'}' | 'print' expr`.
`if`/`while` run/loop while the condition is nonzero (nestable). **`read var`** reads a decimal from stdin into a variable (runtime decimal-parse
routine, like print's). **`print expr` writes
the decimal value of the result followed by a newline** (via a runtime int→decimal
routine emitted once, called per print). The program exits with the value of the last
variable assigned; vars are `a`–`j` in `r6`–`r15`. Comparison operators `< > == <= >= !=`
yield 0/1 (so `while i < n`, `if a == b`). **Parentheses** `(expr)` work to any
nesting depth, via a runtime value stack (saving the sum/term accumulators around the
sub-expression). Source supports `#` line comments. >10 vars not yet (needs memory
slots). Growing next: `else`, then more vars; factor gamma's emission to curb tape size.

```
a = 6 * 7  print a                          -> 42
a = 1  while a < 6 { print a  a = a + 1 }    -> 1 2 3 4 5  (one per line)
```
