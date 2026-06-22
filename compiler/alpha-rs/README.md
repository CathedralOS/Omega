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

## Next slices (grow the grammar feature-by-feature)

3. Host output ("hello"): the import table (kernel32) + a read-only data section
   + the `{ptr,len}` slice descriptor + the x64 calling convention. The big
   unlock — every later host call rides this, including the self-hosting
   compiler's own file read/write.
4. Control flow: `transition` / guards → cmp + jcc, the state/jump model
   (Omega's core execution shape).
5. A second machine + a call: the call-frame / DAG-call model.
