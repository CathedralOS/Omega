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
  emitting the same stack-machine asm shape as the on-ramp.
- **Slice 2a — named locals: DONE.** A real tokenizer (identifiers, keywords,
  `;` comments) and a per-proc symbol table, so `let` locals, assignment, and
  variable references work (`let a = 6 let b = 7 return a * b` → 42). Single pass
  with one pre-scan (`count_lets`) to size the frame before the prologue, since
  single-pass codegen can't know the local count up front.
- **Slice 2b — control flow: DONE.** `if`/`else`, `while`, and the six
  comparisons (`< > == != <= >=`, branch-materialized to 0/1 with generated
  labels). `while i <= n { s = s + i ... }` sums 1..n → 55. 20/20 in `test.sh`.

**The hole is now the hard blocker.** Slice 2b lands bc at **30.7 KB — only ~2 KB
under the 32 KB tape hole**. The next slice (memory, char/string literals, or
procedures/calls) cannot fit. Resolving the hole (the assembler `.db` data section
+ `write_str`, so emitted strings cost ~1 tape byte/char instead of ~12) is the
prerequisite for the remaining slices and for self-hosting. See `TASKS_BOOTSTRAP.md`.

Next slices follow the on-ramp's path: `byte[]`/`word[]` memory, char/string
literals, then procedures + parameters + calls — at which point bc can compile its
own source (self-hosting).

## Known constraint — the 32 KB hole (real, now measured)

`emit("...")` lowers every output byte to an `imm`+`write` pair (~12 tape bytes
per character of emitted asm), and a compiler is mostly fixed output strings, so
`bc`'s own tape grows fast. Two mitigations are in play:

1. **bc emits compact asm** (no indent, no alignment, commas attached — the
   assembler treats those as whitespace). This is a zero-infra, both-seeds-safe
   win that keeps the tape small. Slice 1 is **~12.4 KB** (was ~15.5 KB verbose).
2. The data-section fix (**now required** — slice 2b lands bc at 30.7 KB): add a
   `db "..."` directive to the assembler so fixed output is *data* (1 tape
   byte/char), with a `write_str(addr,len)` loop, instead of an `imm`+`write` per
   byte (~12). This **preserves both-seeds lockstep** (the assembler's own source
   uses no `db`, so its tape — and the self-host fixed point — is unchanged) and
   shrinks emitted tapes ~10×. Enlarging the hole is the cruder alternative but
   needs *both* seeds changed to keep the diamond, and the x64 forge isn't on this
   host — so the data-section route is preferred.

   **De-risking finding (assembler read):** `emit_operand`'s `eo_label` path
   already resolves a label name as an 8-byte immediate, so **`imm rD, Lstr` works
   today with no assembler change**. The remaining work is therefore *only* the
   `db` directive (string tokenization in `next_token` — quotes + escapes, since a
   string can contain spaces/newlines — plus length accounting in pass 1 and byte
   emission in pass 2), then change `emit` lowering in `beta-lang-rs` (and, later,
   in bc's own string-literal slice) to: emit the bytes as `Lstr_N: db "..."` in a
   trailing data section, and at the use site `imm r0,Lstr_N / imm r1,<len> / call
   __write_str`. Verify `../beta/selfhost.sh` stays byte-identical after the
   assembler change.

The slices-1–2b milestone holds regardless; this is the next focused task.
