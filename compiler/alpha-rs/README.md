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

- **Slice 7a — `data` structs + mutable `self` fields: DONE.** `data Name { f: T;
  ... }` lays out scalar fields (8 bytes each; boundary fields = 0; nested data =
  its size). The entry machine's `self` is a zero-initialized (ZII) instance in
  its own long-lived frame, reached through a self-pointer slot; methods get
  `self` in rcx. `self.field` reads (`mov eax,[self+off]`) and `self.field = e`
  writes are scalar-only for now. Verified: mutable fields, ZII reads 0, a counter
  held in `self` across a loop, multi-field offsets. See `samples/data.alpha`.

- **Slice 7b — array fields + trap-checked indexing: DONE.** `data` fields may be
  `[scalar; N]` (N×8 bytes). `self.arr[i]` reads and `self.arr[i] = e` writes
  compute `self_ptr + field_offset + i*8` after a `cmp i,N; jb +2; ud2` bounds
  check — an out-of-bounds index **traps** (SIGILL), per the Alpha spec. The index
  is any expression. Verified: write/read, fill-and-sum in a loop with a runtime
  index, OOB traps; `samples/arena.alpha` computes fib(11)=89 in an array.

- **Slice 8a — host byte I/O: DONE.** `read_byte() -> i32` (next stdin byte, or
  -1 at EOF) and `write_byte(b)` (one byte to stdout), via `ReadFile`/`WriteFile`
  on the std handles (`ReadFile` added as a third kernel32 import; the import
  table is now generated from a name list). EOF is branchless (`cmove`). Verified:
  `echo.alpha` cats stdin→stdout; a byte counter; `write_byte` emits `ABC`. This
  is the I/O backbone — the self-hosting compiler reads source and writes output
  bytes through these.

- **Slice 8b — `[u8;N]` byte buffers: DONE.** Array fields now carry an element
  width (`u8` = 1 byte, other scalars = 8). Indexing scales by it (`shl` by 0/2/3)
  and uses a byte (`movzx`/`mov [rax],cl`) or dword access accordingly — the bounds
  check is unchanged. So source/output can be buffered in a `data` field. Verified:
  buffered cat round-trips, `buffer.alpha` reverses stdin via a `[u8;4096]` field,
  and i32 arrays (`arena`) still work.

The on-ramp now has a compiler's full vocabulary: arithmetic, control flow + loops,
a DAG of machines with params/returns, structs with mutable `self` fields, scalar
and byte arrays with trapping indexing, and byte stdin/stdout I/O.

- **Slice 9 — `&mut self` method calls on data: DONE.** `self.method(args)` calls
  another method of the same machine, passing the current self-pointer in rcx and
  args in rdx/r8/r9 (≤3 args, since rcx is self). Works in value position
  (`let a = self.pop()`) and as a statement (`self.push(5);`, via a new `Eval` node
  that discards the result). Mutations persist across calls because all methods
  share one self instance. Verified: counter methods (12), array push/read (44),
  method-of-method (7); `methods.alpha` runs a self-resident stack → 75.

The on-ramp is now feature-complete for writing a compiler: a `Main` holds the
arenas/buffers as self fields, and lexer/parser/emitter helper machines mutate
them through `self.*` method calls.

## Next

10. Write the Alpha compiler **in Alpha** under `compiler/alpha/` (lexer → parser →
    layout → x64 + PE emitter, all as methods over a `Main` holding the source
    buffer + arenas), bootstrap it through `alpha-rs`, and close the self-hosting
    fixed point. Sum-types-as-tags is optional (token/AST kinds = i32 tags
    dispatched by `transition`). De-recurse the tree-walks to an explicit worklist
    (Alpha bans call recursion).

Deferred subset-enforcement (front-end is the spec; add before self-host): reject
a cyclic call graph (Alpha bans recursion — calls must be a DAG); >4-arg calls
(stack args); arena-capacity bounds.

Deferred subset-enforcement (front-end is the spec; add before self-host): reject
a cyclic call graph (Alpha bans recursion — calls must be a DAG); >4-arg calls
(stack args); arena-capacity bounds.
