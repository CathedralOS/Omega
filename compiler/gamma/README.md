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

**Status: v4** — variables, statements, and control flow (`if`):
`statement := var '=' expr | 'if' expr '{' statement* '}'`. An `if` runs its body when
the condition is nonzero; ifs nest (label numbers `L<n>e` via a counter + a label-number
stack). The program exits with the value of the last variable assigned. Expressions have
precedence (recursive descent, two accumulators); factors are numbers or variables
(`a`–`j` in `r6`–`r15`). New: `emit_dec` (int→decimal). No parens / >10 vars yet (need a
value stack / memory slots). Growing next: `while`, comparison operators, `print`.
