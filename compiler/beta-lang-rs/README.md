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

## Next

Beta is now *more* capable than the assembler ever needed (which was hand-written
in raw Alpha asm with none of this), so it is plausibly already compiler-grade.
The remaining steps are ergonomics + validation, not raw capability:

5. Ergonomics for compiler-writing — char literals (`'a'`), and likely fixed-address
   globals / `>4` args only if the self-check demands them.
6. **Self-check:** write something compiler-shaped in Beta (a tokenizer / tiny
   expression evaluator) to confirm it is genuinely pleasant.
7. Transcribe the trusted compiler to Alpha assembly (the one unavoidable time),
   cross-check against this on-ramp, then rewrite gamma **in Beta**.

See [`../beta/LANGUAGE.md`](../beta/LANGUAGE.md) for the language surface.
