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

**Status: v5** — variables, statements, and control flow (`if` **and `while`**):
`statement := var '=' expr | 'if' expr '{' …'}' | 'while' expr '{' …'}'`. Both run/loop
while the condition is nonzero; they nest (label numbers via a counter + a label-number
stack). `while` emits `L<n>t: <cond> jz L<n>e <body> jmp L<n>t L<n>e:`. The program exits
with the value of the last variable assigned. Expressions have precedence; factors are
numbers or variables (`a`–`j` in `r6`–`r15`). No parens / >10 vars yet (need a value
stack / memory slots). Growing next: comparison operators (`<`, `==`), then `print`.
