# compiler/alpha-rs — the Alpha compiler (interim Rust)

Sibling of `compiler/omega-rs`. Alpha is the smallest subset of Omega (rung 0 of
the self-building lattice). The `-rs` suffix marks this as the **throwaway Rust**
implementation (binary `alphac`) — the "on-ramp." Eventually a `compiler/alpha`
holds the Alpha seed + the Alpha-in-Alpha compiler, and this Rust on-ramp is
discarded (just as `compiler/omega-rs` is the throwaway Rust Omega compiler,
to be retired by a lattice-built `compiler/omega`).
See `wiki/design_briefs/alpha_language.md` and the `self-building-lattice` notes.

Its job is to *discover what Alpha needs by compiling it*, then be ported 1:1 to
Alpha so Alpha compiles itself. Its trust lineage does not matter (it is
discarded); it is written in deliberately dumb, arena/index-based, monomorphic
Rust so the port is mechanical, and its front-end enforces the Alpha subset.

Standalone crate (own `[workspace]`), so the parent Omega workspace never absorbs
it and it never rots when the main compiler churns.

## Run

```
cargo run -- samples/exit7.alpha out.exe
./out.exe ; echo $?     # -> 7
```

## Status

- **Slice 1 — `exit_process(N)` end-to-end: DONE.** Lex → parse → lower → emit a
  Windows x64 PE that exits with the given code. Deterministic. Minimal PE: no
  imports; `exit_process(N)` lowers to `mov eax,N; ret`, relying on the Windows
  thread stub to exit with the entry's return value.
- **Slice 2 — expressions + locals: DONE.** `let x: i32 = 3 + 4 * 2;
  exit_process(x)`. Real statement/expression parser (precedence climbing),
  index-based expr arena, locals in an rbp frame, stack-machine codegen,
  `+ - * /`. Verified: precedence (`3+4*2`→11), multi-locals (`(10-3)*2`→14),
  deterministic, and **trap-on-overflow** (i32 overflow → `ud2`/Illegal
  instruction — the "trap everything" decision enforced in codegen; `/` traps on
  div-by-zero via hardware `idiv`).
- **Slice 3 — host output: DONE.** `write_line("...")` → a `.rdata` section + a
  kernel32 import table (IDT/ILT/IAT/hint-name) + `GetStdHandle`/`WriteFile`
  lowered to the full x64 ABI (rcx/rdx/r8/r9 + 32-byte shadow + 5th arg + 16-byte
  alignment), with RIP-relative relocations patched by the PE writer (now a
  generic multi-section assembler). Verified: `hello, alpha` prints; multiple
  `write_line`s; deterministic; slices 1-2 unregressed (single-section path
  byte-identical). This is the I/O backbone — the self-hosting compiler reads
  source and writes its output through this same machinery.

- **Slice 4 — comparisons + control flow: DONE.** `< > <= >= == !=` (signed
  cmp/setcc → 0/1). `transition <expr> { <pat> -> state() ... }` over int / `true`
  / `false` / `_` patterns, with `state name() { ... }` blocks lowered to labels +
  `cmp`/`je`/`jmp` (intra-text relocations patched after layout; backward jumps =
  loops). Verified: 3-arm int dispatch (1→11, 2→22, default→99), boolean
  transition, no regression. This is Omega's core execution model.

- **Slice 5 — mutable reassignment + loops: DONE.** `x = e` reassigns a declared
  local (stores to its existing frame slot); combined with the slice-4 back-edge
  transition this gives real loops. Verified: a counter loops 0→3 and exits 3; a
  loop with a `write_line` side effect prints `tick` three times. See
  `samples/loop.alpha`.

- **Slice 6 — machine calls (the DAG model): DONE.** A program is now a list of
  callable machines. Free machines take value params (`a: i32`) and `return` a
  value; calls in value position (`add(2, 3)`) pass args in the Win64 registers
  (rcx/rdx/r8/r9, ≤4 for now), `call rel32` to the callee, result in eax. Each
  machine is its own function (own frame, params spilled to slots on entry); the
  entry machine is lowered first so the PE entry stays at offset 0; cross-machine
  call rel32s are patched after layout. Forward references work (machine names are
  pre-scanned), and a callee may have its own transitions. Verified: `add(20,22)`,
  forward-ref `square`/`mul`, `max(max(7,19),12)`, 4-arg `sum4` feeding
  arithmetic. See `samples/calls.alpha`.

## Next slices (grow the grammar feature-by-feature)

7. `data` structs + field access (incl. `&mut self` methods); `[T;N]` arrays +
   trap-checked indexing (the IR arenas — the heart of the compiler's storage).
8. File I/O: CreateFile/ReadFile/WriteFile — the compiler reads a source file and
   writes a `.exe`. Then sum-types-as-tags, and the Alpha compiler in `compiler/alpha/`.

Deferred subset-enforcement (front-end is the spec; add before self-host): reject
a cyclic call graph (Alpha bans recursion — calls must be a DAG); >4-arg calls
(stack args); arena-capacity bounds.
