# Delta Rust on-ramp

The `-rust` suffix marks this as the external-language on-ramp for the Delta rung
(`-rs` survives only in historical compatibility paths):
it compiles the current experimental `.alp` surface (state **machines**, **data**
structs, **transition** dispatch, and payload **enums**) to a native binary to
discover what the bridge language needs. It is an interim producer and
differential oracle, not Delta's semantic or feature authority; canonical
meaning is the Delta-to-Gamma route. Acceptance here does not admit a construct
to Delta v1. The implementation is deliberately direct, arena/index-based, and
monomorphic so the port down to the lattice is mechanical.

> **Naming note.** Header, extension (`.alp`), and a few "Alpha" mentions in older
> samples are inherited from the `alpha-rs` README this was forked from — they do
> **not** mean this is the 21-opcode tape-VM *alpha* (owned by
> `bootstrap/rungs/alpha/`; `compiler/alpha` is a compatibility path).
> This builds the richer machines/data/transition language and is gated as
> **Delta** in `verify-lattice.sh`.

The Rust-free self-hosting compiler artifact is canonically owned by the Delta
rung at **`../../rungs/delta/samples/lowermachine.alp`**. The local `samples`
entry is a compatibility symlink so old commands keep working.
It compiles itself to a byte-identical binary (the dependency-closure fixed point,
gated by `test_aarch64.sh` / `convergence.sh`). The fixed point establishes
reproducibility, not semantic correctness.

Two backends: Windows x64 PE (`src/x64.rs`, the default) and macOS arm64 Mach-O
(`src/aarch64.rs`, `DELTA_ARCH=aarch64`, the runnable+gated one on this platform).
Standalone crate (own `[workspace]`), so the parent Omega workspace never absorbs it.

## Run

```
cargo run -- ../../rungs/delta/samples/exit7.alp out.exe  # Windows x64 PE
./out.exe ; echo $?                             # -> 7
DELTA_ARCH=aarch64 cargo run -- ../../rungs/delta/samples/shape.alp out  # macOS arm64
./out ; echo $?                                 # -> 42
```

## Status

- **Bridge frontend O1 — DONE for native/self-host, direct artifacts, and the
  current lower-rung observations.**
  `../../omega-bootstrap/compiler/omega-bootstrap-frontend.alp` is the canonical
  first Delta-written bridge compiler slice
  (`samples/omega-bootstrap-frontend.alp` is the canonical sample link and
  `samples/omega0-frontend.alp` is a compatibility alias):
  canonical one-source bundle decoding, checked source storage, complete UTF-8
  validation, streaming lexing, and exact O0/O1 parsing/name/type/count checks,
  retaining 0–16 ordered `write_line` literals plus one final `exit_process`
  `i32`. Its focused gate covers the acceptance/rejection matrix and recompiles
  the frontend through Delta-written `lowermachine`; every operand affects the
  observed success digest. The complete 40-machine O1 frontend also elaborates
  through the Beta-written `omega2gamma.beta` and Gamma's canonical interpreter,
  pinning canonical, zero-write, two-write, rejection, and multi-slot method-
  state observations. An exact native-versus-Gamma comparison of every terminal
  byte and exhaustion result remains follow-up work. Direct canonical terminal-
  Psi emission is complete and checked by the shared decoder/verifier. Its output is published
  only through same-directory staging, persistence, canonical decode, expected
  identity binding, and atomic rename. Resulting O1 fixtures are checked from
  canonical terminal meaning through deterministic Linux x86-64/AArch64 images,
  installation replay, and native execution on a matching Linux host.

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
  `samples/loop.alp`.

- **Slice 6 — machine calls (the DAG model): DONE.** A program is now a list of
  callable machines. Free machines take value params (`a: i32`) and `return` a
  value; calls in value position (`add(2, 3)`) pass args in the Win64 registers
  (rcx/rdx/r8/r9, ≤4 for now), `call rel32` to the callee, result in eax. Each
  machine is its own function (own frame, params spilled to slots on entry); the
  entry machine is lowered first so the PE entry stays at offset 0; cross-machine
  call rel32s are patched after layout. Forward references work (machine names are
  pre-scanned), and a callee may have its own transitions. Verified: `add(20,22)`,
  forward-ref `square`/`mul`, `max(max(7,19),12)`, 4-arg `sum4` feeding
  arithmetic. See `samples/calls.alp`.

- **Slice 7a — `data` structs + mutable `self` fields: DONE.** `data Name { f: T;
  ... }` lays out scalar fields (8 bytes each; boundary fields = 0; nested data =
  its size). This backend currently lowers an attached entry machine's `self` as
  a zero-initialized (ZII) instance in its own long-lived frame, reached through
  a self-pointer slot; methods get `self` in rcx. That frame is an implementation
  strategy, not an ambient language `static`: the source model requests exactly
  one target-provisioned receiver by binding the attached entry machine.
  `self.field` reads (`mov eax,[self+off]`) and `self.field = e` writes are
  scalar-only for now. Verified: mutable fields, ZII reads 0, a counter held in
  `self` across a loop, multi-field offsets. See `samples/data.alp`.

- **Slice 7b — array fields + trap-checked indexing: DONE.** `data` fields may be
  `[scalar; N]` (N×8 bytes). `self.arr[i]` reads and `self.arr[i] = e` writes
  compute `self_ptr + field_offset + i*8` after a `cmp i,N; jb +2; ud2` bounds
  check — an out-of-bounds index **traps** (SIGILL), per the Alpha spec. The index
  is any expression. Verified: write/read, fill-and-sum in a loop with a runtime
  index, OOB traps; `samples/arena.alp` computes fib(11)=89 in an array.

- **Slice 8a — host byte I/O: DONE.** `read_byte() -> i32` (next stdin byte, or
  -1 at EOF) and `write_byte(b)` (one byte to stdout), via `ReadFile`/`WriteFile`
  on the std handles (`ReadFile` added as a third kernel32 import; the import
  table is now generated from a name list). EOF is branchless (`cmove`). Verified:
  `echo.alp` cats stdin→stdout; a byte counter; `write_byte` emits `ABC`. This
  is the I/O backbone — the self-hosting compiler reads source and writes output
  bytes through these.

- **Slice 8b — `[u8;N]` byte buffers: DONE.** Array fields now carry an element
  width (`u8` = 1 byte, other scalars = 8). Indexing scales by it (`shl` by 0/2/3)
  and uses a byte (`movzx`/`mov [rax],cl`) or dword access accordingly — the bounds
  check is unchanged. So source/output can be buffered in a `data` field. Verified:
  buffered cat round-trips, `buffer.alp` reverses stdin via a `[u8;4096]` field,
  and i32 arrays (`arena`) still work.

The on-ramp now demonstrates a broad candidate compiler vocabulary: arithmetic,
control flow and loops, a DAG of machines with parameters and returns, structs
with mutable `self` fields, scalar and byte arrays with trapping indexing, and
byte stdin/stdout I/O.

- **Slice 9 — `&mut self` method calls on data: DONE.** `self.method(args)` calls
  another method of the same machine, passing the current self-pointer in rcx and
  args in rdx/r8/r9 (≤3 args, since rcx is self). Works in value position
  (`let a = self.pop()`) and as a statement (`self.push(5);`, via a new `Eval` node
  that discards the result). Mutations persist across calls because all methods
  share one self instance. Verified: counter methods (12), array push/read (44),
  method-of-method (7); `methods.alp` runs a self-resident stack → 75.

The on-ramp now demonstrates enough candidate machinery to write a substantial
compiler: a `Main` holds arenas/buffers as self fields, and
lexer/parser/emitter helper machines mutate them through `self.*` method calls.
**`lowermachine.alp` self-compiles** (the byte-identical fixed point), so the
Rust on-ramp is discardable from steady state. This does not show that every
demonstrated feature belongs in the final bridge or Delta v1.

`samples/bootstrap-storage.alp` fixes the current D0 bridge storage convention over
that surface: runtime-sized, aligned reservations return integer offsets into an
explicit fixed backing array; exhaustion preserves allocator state; and a mark
and reset pair provides bulk reclamation. `lowermachine.alp` now uses that shape
directly: source input grows one checked cell at a time in an explicit byte
backing, while offset handles carve its compiler tables from one reserved typed
backing extent. Exhaustion is an explicit exit failure instead of silent tail
truncation. The frozen D0 contract is recorded in
[`../../omega-bootstrap/compiler/BOOTSTRAP_PROFILES.md`](../../omega-bootstrap/compiler/BOOTSTRAP_PROFILES.md).
`delta-storage-meaning.sh` runs the exact contract and a perturbation through
the lower-rung `omega2gamma.beta` → `interp.beta` route without using the Rust
Gamma emitter.

## Experimental additions beyond slice 9

Each is accepted by both reference backends and keeps the self-compile fixed
point byte-identical (additive—existing programs lower unchanged). They are
discovery experiments, not ratified Delta-v1 language additions.

- **Operators.** `%` (remainder, traps on `/0` like `/`); bitwise `& | ^`; shifts
  `<< >>` (arithmetic right, shift amount mod-32 on both backends); unary minus
  `-x` (desugars to `0 - x`, so it reuses the overflow trap). Precedence: bit
  operators share one level below comparison; `* / %` above `+ -` above comparison.
  Samples: `modulo.alp`, `bitops.alp`, `shifts.alp`, `negate.alp`.

- **Enums (`case`) — Omega's sum types.** `data E { case A; case B(x: i32); ... }`.
  A value is a tag word (the variant index) plus, for payload variants, one word
  per payload field at offset +8, +16, … (the enum is sized to the widest variant).
  - construct: `self.e = E::B(5)` (or `E::Rect(w, h)`); a tag-only `E::A` is just the tag.
  - match: `transition self.e { E::A -> s() ; E::B { x } -> s2(x) ; ... }` dispatches
    on the tag and binds payload fields (reads of the matched value) for the arm.
  - **exhaustiveness:** a transition over an enum field must cover every variant or
    include `_`, else it is rejected (naming the missing variant).
  - Samples: `enum.alp`, `payload.alp`, `shape.alp`.

- **State parameters.** `state s(p: i32, ...) { ... }` takes value args; a transition
  arm `pat -> s(args)` evaluates them into the machine's shared state-arg slots, then
  branches. Lets a loop carry an accumulator without a `self` field, and is the
  mechanism enum-payload bindings reuse. Sample: `stateparams.alp`.

## Next

- Add mixed field-plus-case data only if the real bridge demonstrates that
  separate records and sums impose greater total cost. Producer completeness is
  not independently a goal.
- **Subset enforcement** the front end should add as it firms up: arena-capacity
  bounds; `>4`-arg free calls already error. The Gamma meaning route remains the
  semantic authority.
- Retire the remaining inherited `alpha-rs` framing as the Delta surface firms up.
- Grow the real bridge while maintaining a provisional Delta feature ledger;
  after the complete source closure exists, remove unused experiments and
  freeze Delta's smallest robust literal compiler-host specification. It must
  implement `omega-bootstrap` with exact `Ωself` acceptance.
  The bridge may conservatively lower the production compiler, but it must
  correctly compile the `Ωself` source that implements the full optimizer and
  advanced lowering. Those product passes need not be implemented twice.
