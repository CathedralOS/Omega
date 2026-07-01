# `compiler/beta-lang-rs/` — the Beta compiler, throwaway Rust on-ramp

This is the **Beta language** compiler (not the assembler) as a temporary Rust
on-ramp. It reads `.beta` source and emits **Alpha assembly**; the existing
assembler (`../beta`) lowers that to a tape, which the seed runs.

Its only job is to discover and pin the Beta language *ergonomically* — Rust lets
us iterate the design to an elegant shape instead of hand-editing assembly. The
**trusted** Beta compiler is later transcribed into Alpha assembly (the one
unavoidable assembly compiler) and cross-checked against this; then this crate is
discarded, exactly as `alpha-rs`/`beta-rs` are. It is deliberately dumb,
index/arena-based, monomorphic Rust so that port is mechanical.

> Naming: `beta-rs` is the on-ramp for the **assembler**; `beta-lang-rs` is the
> on-ramp for the **language compiler**. (The long-view cleanup — the assembler is
> really Alpha's assembler, freeing "beta" to mean the language — is deferred.)

## Build / run

```
./build.sh examples/double.beta     # .beta -> asm -> tape -> build/double.exe
./build/double.exe ; echo $?        # -> 42
```

`build.sh` runs `cargo run` to produce the assembly, then reuses the assembler +
seed-stamp. Needs `cargo`.

## Status (incremental, like the alpha-rs/beta-rs slices)

- **Slice 1 — arithmetic: DONE.** `proc main() { return <expr> }` with
  `+ - * / %` and parentheses, lowered onto the data stack. `answer.beta` → 42.
- **Slice 2 — procedures, parameters, calls: DONE.** Multiple `proc`s, ≤4
  params, calls in expressions, parameters addressed via the frame pointer — i.e.
  the [calling convention](../beta/CALLING_CONVENTION.md) generated mechanically.
  `double.beta` → 42, `calls.beta` (nested `add(mul(2,3),4)`) → 10.
- **Slice 3 — control flow + locals: DONE.** Multi-statement bodies, `let` locals
  (function-scoped frame slots), assignment, `if`/`else` and `while` (→ `jz`/`jmp`),
  and the six comparisons (`< > == != <= >=`, materialized to 0/1). **Unlocks
  recursion + loops:** `factorial.beta` → 120 and `fib.beta` → 55 match the hand
  `.alpha` proofs; `sumto.beta` (while + let) → 55.
- **Slice 4 — explicit memory: DONE.** `byte[addr]` / `word[addr]` load and store,
  lowered to `loadb`/`load`/`storeb`/`store`. Raw arrays/buffers, addresses managed
  by the programmer (above the data stack). `arrays.beta` (fill + sum `i*i`) → 30,
  `bytes.beta` → 131.
- **Slice 5 — ergonomics + the host boundary: DONE.** Char literals (`'a'`, with
  `\n \t \r \0 \\ \'` escapes) so text code reads in characters, not magic byte
  numbers; the `read_byte()` / `write_byte(x)` intrinsics (the only host boundary,
  lowering straight to Alpha `read`/`write`); call-as-statement (`f(x)` for
  effect, result discarded — grammar-listed but previously unimplemented); and
  `emit("text")` (a string literal written byte-by-byte — no string type, just so
  a Beta-written compiler can emit fixed output like asm mnemonics).
- **Slice 6 — the self-check: DONE.** [`calc.beta`](examples/calc.beta) is a
  **recursive-descent calculator written in Beta**: it reads an arithmetic
  expression from stdin (decimal ints, `+ - * /`, parentheses, precedence,
  whitespace), evaluates it, prints the decimal result, and returns it as the exit
  code. It exercises the whole surface — char literals, the I/O boundary, a
  memory-backed input buffer + cursor, and recursion through the grammar. This is
  the proof Beta is *compiler-grade*: a real parser/evaluator is pleasant to write
  in it. `2+3*4`→14, `(2+3)*4`→20, `2*(3+4)*5`→70. (calc tape ≈ 6.7 KB.)

## Next

The self-check passed — Beta is compiler-grade. The remaining lattice steps:

7. **Transcribe** the trusted compiler to Alpha assembly (the one unavoidable time
   we hand-write a structured-language compiler in asm), cross-check against this
   on-ramp (a diamond), and discard the Rust.
8. **Rewrite gamma in Beta**, retiring `gamma.alpha`.

See [`../beta/LANGUAGE.md`](../beta/LANGUAGE.md) for the language surface, and run
`sh test.sh` to verify the whole compiler end to end (8 examples + 9 calc cases).
