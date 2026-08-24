# Beta Rust producer

This is the retained Rust diagnostic/reference compiler for the **Beta language**
(not the assembler). It reads `.beta` source and emits **Alpha assembly**; the
Alpha assembler (`../../rungs/alpha/assembler/`) lowers that to a tape, which
the seed runs.

Its original job was to discover and cold-start the language. The Alpha-written
compiler under `../../rungs/beta/cold-start/` now reconstructs the persisted
self-hosted artifact without Rust, and downstream gates consume that artifact.
This crate is no longer a bootstrap dependency. It remains useful as an explicit
diagnostic/reference producer while complete lower-rooted validation of the
Alpha-rooted `bc` artifact is built. Successful production and a fixed point
prove neither source-to-artifact correctness nor semantic authority.

> Naming: `bootstrap/onramps/beta-rust` is the canonical Beta-language producer.
> The former `compiler/beta-lang-rs` and `compiler/beta-rs` compatibility
> entries have been retired; the Alpha assembler producer is canonically
> `bootstrap/onramps/alpha-assembler-rust`.

## Build / run

```
./build.sh examples/double.beta     # .beta -> asm -> tape -> build/double.exe
./build/double.exe ; echo $?        # -> 42
```

`build.sh` runs `cargo run` to produce the assembly, then reuses the assembler +
seed-stamp. Needs `cargo`.

## Historical implementation slices

- **Slice 1 — arithmetic: DONE.** `proc main() { return <expr> }` with
  `+ - * / %` and parentheses, lowered onto the data stack. `answer.beta` → 42.
- **Slice 2 — procedures, parameters, calls: DONE.** Multiple `proc`s, ≤4
  params, calls in expressions, parameters addressed via the frame pointer — i.e.
  the [calling convention](../../rungs/beta/CALLING_CONVENTION.md) generated mechanically.
  `double.beta` → 42, `calls.beta` (nested `add(mul(2,3),4)`) → 10.
- **Slice 3 — control flow + locals: DONE.** Multi-statement bodies, `let` locals
  (function-scoped frame slots), assignment, `state` basic blocks, guarded or
  unconditional `to` transitions (→ `jz`/`jmp`), and the six comparisons
  (`< > == != <= >=`, materialized to 0/1). **Unlocks recursion + loops:**
  `factorial.beta` → 120 and `fib.beta` → 55 match the hand `.alpha` proofs;
  `sumto.beta` (a state loop with a local accumulator) → 55.
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

## Current role

The self-check passed and the Beta-written compiler superseded this on-ramp.
Changes to the Beta surface must update `bc.beta`, its language gates, and the
canonical Beta meaning/refinement route.

See [`../../rungs/beta/LANGUAGE.md`](../../rungs/beta/LANGUAGE.md) for the language surface, and run
`sh test.sh` to verify the retained example and calculator corpus end to end.
