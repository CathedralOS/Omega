# `compiler/beta-lang/` — the Beta compiler, written in Beta

This is **slice 7 of the lattice**: the Beta-language compiler, written *in Beta
itself* (`bc.beta`), rather than in the throwaway Rust on-ramp (`../beta-lang-rs`)
or hand-written in assembly. It is the self-hosting path — once `bc.beta` compiles
its own source to a fixed point, Rust leaves the trusted lineage entirely (it only
cold-starts the first `bc`, exactly as `beta-rs` cold-starts the assembler).

```
bc.beta     the Beta compiler, in Beta:  reads .beta on stdin, emits Alpha asm on stdout
test.sh     the gate: build bc via the on-ramp, then use bc to compile + run programs
```

## How it bootstraps

```
  bc.beta ──(beta-lang-rs, the Rust on-ramp)──▶ asm ──(assembler)──▶ bc.exe
  program.beta ──(bc.exe)──▶ asm ──(assembler)──▶ tape ──▶ run
```

`bc.exe` is a real Beta compiler with no Rust in *its* execution — only in the
one-time lowering of `bc.beta`'s own text. The endgame: `bc.exe` compiles
`bc.beta` to a tape `T1`, and `T1` compiles `bc.beta` to `T2` with `T1 == T2` — a
self-hosting fixed point, just like `../beta/selfhost.sh` for the assembler. At
that point the Rust on-ramp is discardable.

Run the gate:

```sh
sh test.sh        # builds bc, compiles arithmetic programs with it, checks results
```

## Status — built slice by slice (mirroring `beta-lang-rs`)

- **Slice 1 — arithmetic: DONE.** `proc main() { return <expr> }` with `+ - * / %`,
  parentheses, precedence, over integer literals. Recursive-descent codegen
  emitting the same stack-machine asm shape as the on-ramp. 8/8 in `test.sh`
  (`6*7`→42, `2+3*4`→14, `(2+3)*4`→20, `2*(3+4)*5`→70, `100/7`→14, `17%5`→2).

Next slices follow the on-ramp's path: procedures + calls, `if`/`while` + locals,
`byte[]`/`word[]` memory, then a real tokenizer with a name/symbol table (so it
can parse arbitrary identifiers, not just the fixed slice-1 prologue) — at which
point it can compile its own source.

## Known constraint — the 32 KB hole (real, now measured)

`bc.beta` at **slice 1 already lowers to a ~15.5 KB tape** — and that is *only*
arithmetic. The cost is `emit("...")`: every output byte lowers to an `imm`+`write`
pair (~12 bytes of tape per character of emitted asm), and a compiler is mostly
fixed output strings. The full self-hosting compiler will comfortably exceed the
seed's **fixed 32 KB tape hole** (flagged in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md) — "the Beta compiler's own tape
will eventually outgrow it"). This is now the concrete blocker for finishing
slice 7, and it forces one of:

- **make the tape hole an execution parameter** (the flagged small VM change —
  rebuild both seeds), and/or
- **a compact string-emit** (e.g. an assembler `.db` data directive + a
  `write_str(addr,len)` loop, so fixed output is *data* looped over rather than
  one `imm`+`write` per byte) — which would shrink emitted tapes dramatically and
  is independently worthwhile.

The slice-1 milestone holds regardless; this is the next thing to resolve before
the compiler can grow to self-hosting size.
