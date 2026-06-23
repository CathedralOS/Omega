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

**Status: v6** — variables, arithmetic (with precedence), `if`, `while`, and `print`:
`statement := var '=' expr | 'if' expr '{'…'}' | 'while' expr '{'…'}' | 'print' expr`.
`if`/`while` run/loop while the condition is nonzero (nestable). `print expr` writes the
low byte of the result (one character) to stdout — so programs produce visible output,
not just an exit code. The program exits with the value of the last variable assigned;
vars are `a`–`j` in `r6`–`r15`. **Comparison operators** `<` `>` `==` form a precedence
level above `+`/`-` and yield 0/1 (so `while i < n { … }`, `if a == b { … }`). No parens
/ >10 vars yet. Growing next: decimal-number printing, then parens + a value stack.

```
print 72  print 105  print 10            -> Hi
a = 5  while a { print 42  a = a - 1 }    -> *****
```
