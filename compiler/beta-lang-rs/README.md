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

## Next slices

3. `if` / `while` + multi-statement bodies + `let` locals → unlocks **recursion**
   (re-derive `factorial`/`fib` as `.beta`, matching the hand-written `.alpha`).
4. Explicit memory (`byte[]` / `word[]`) → arrays/buffers.
5. A symbol table (named globals/procs beyond the flat list).
6. Self-check: write something compiler-shaped in Beta; then transcribe the
   trusted compiler to assembly and rewrite gamma **in Beta**.

See [`../beta/LANGUAGE.md`](../beta/LANGUAGE.md) for the language surface.
