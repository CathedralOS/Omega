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

`emit("...")` lowers every output byte to an `imm`+`write` pair (~12 tape bytes
per character of emitted asm), and a compiler is mostly fixed output strings, so
`bc`'s own tape grows fast. Two mitigations are in play:

1. **bc emits compact asm** (no indent, no alignment, commas attached — the
   assembler treats those as whitespace). This is a zero-infra, both-seeds-safe
   win that keeps the tape small. Slice 1 is **~12.4 KB** (was ~15.5 KB verbose).
2. Still, the *full* self-hosting compiler may exceed the seed's **fixed 32 KB
   tape hole** (flagged in [`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md)). If it
   does, the order-of-magnitude fix is a **data section** — an assembler `.db`
   directive + a `write_str(addr,len)` loop, so fixed output is *data* (1 tape
   byte/char) looped over instead of an `imm`+`write` per byte. (Enlarging the
   hole is the cruder alternative, but it must be done on *both* seeds to keep the
   diamond; the x64 forge isn't available here, so the data-section route is
   preferred.)

The slice-1 milestone holds regardless; compact emit buys the runway to grow
several more slices before #2 is forced.
