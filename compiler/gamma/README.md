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

**Status: v2** — integer arithmetic expressions with precedence (`* /` bind tighter than
`+ -`), via recursive descent (`expr := term (('+'|'-') term)*`, `term := factor …`) on
the VM call stack; codegen uses two accumulators (`r0` sums, `r4` products). No parens
yet (that needs a runtime value stack). Growing next: variables → `if`/`while` → `print`.
