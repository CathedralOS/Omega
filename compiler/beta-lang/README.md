# `compiler/beta-lang/` — the Beta compiler, written in Beta (SELF-HOSTING)

This is **slice 7 of the lattice, done**: the Beta-language compiler written *in
Beta itself* (`bc.beta`), not in the throwaway Rust on-ramp (`../beta-lang-rs`) and
not hand-written in assembly. **It self-hosts** — `bc` compiles its own source to a
compiler that reproduces that compilation byte-for-byte (`selfhost.sh`), so from
that compiler on, Rust is out of the lineage; it only cold-started the first `bc`,
exactly as `beta-rs` cold-starts the assembler.

```
bc.beta       the Beta compiler, in Beta:  reads .beta on stdin, emits Alpha asm
selfhost.sh   THE gate: bc compiles bc.beta -> bc1; assert bc1(bc.beta) == bc(bc.beta)
test.sh       per-feature gate: bc compiles + runs small programs across slices 1-6
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

## Status — SELF-HOSTING (slices 1–6 done)

bc implements the whole Beta language and compiles its own source to a byte-for-byte
fixed point. Built slice by slice, mirroring `beta-lang-rs`:

| Slice | Adds | Notes |
| --- | --- | --- |
| 1  | arithmetic (`+ - * / %`, parens, precedence) | stack-machine codegen, same shape as the on-ramp |
| 2a | `let` locals, assignment, variable refs | tokenizer + per-proc symbol table; a pre-scan (`count_lets`) sizes the frame |
| 2b | `if`/`else`, `while`, the six comparisons | generated labels; 0/1 materialization |
| 3  | procedures, parameters, calls, recursion | the calling convention (args r0..r3, frames via fp) |
| 4  | `byte[]`/`word[]` memory | `loadb`/`load`/`storeb`/`store` |
| 5  | char literals `'x'`, `read_byte`/`write_byte`, call statements | + prescan skips char/string literals |
| 6  | string literals via `emit("...")` | inline `db` data jumped over + a `__write_str` loop |

`sh selfhost.sh` is the proof; `sh test.sh` is the per-feature gate. bc's self-tape
is ~45 KB — well within the 256 KB arm64 hole (see below).

### The hole (resolved)

`emit` lowers each output byte to ~12 tape bytes, and a compiler is mostly fixed
output, so bc's tape grew fast. Two things were done: the assembler gained a `db`
data section (1 byte/char) — but that only trimmed ~12%, because **bc's tape is
dominated by its own compiled *logic*, not emit strings**. So the arm64 tape hole
was grown 32 KB → **256 KB** (`.space 0x40000`; `HOLE_SIZE` in `seed_env.sh` is now
per-platform). The x64 hole stays 32 KB until a forge rebuild matches it — a
flagged, capacity-only asymmetry (the diamond holds for any tape that fits both;
the self-hosting bc tape is arm64-runnable now, x64 after that one-line catch-up).

<details><summary>historical: the compact-asm + db steps</summary>

bc emits compact asm (no indent/alignment — the assembler treats commas/indent as
whitespace; slice 1 was ~12.4 KB vs ~15.5 verbose), and `emit("...")` lowers to a
`db` data section + `__write_str` loop. Both preserve both-seeds lockstep (the
assembler's own source uses no `db`, so its tape — and the self-host fixed point —
is unchanged). `imm rD, label` already worked (the assembler's `eo_label` path
resolves a label as an 8-byte immediate), so the only assembler change needed was
the `db` directive itself.

</details>

## Next

The Beta rung is self-sufficient. Up the lattice: rewrite **gamma in Beta**
(retiring `gamma.alpha`), then **Delta** — the checker / evidence rung, where trust
actually starts. The Rust on-ramp (`../beta-lang-rs`) is now discardable from the
steady state; keep it only as the documented cold-start.
