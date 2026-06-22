# `compiler/alpha/` — the Alpha compiler, written in Alpha

This is the real bootstrap target: a compiler for the Alpha language, *written in
Alpha*, compiled by the throwaway Rust on-ramp (`../alpha-rs`). When it can compile
its own source and the result compiles its own source to a byte-identical binary,
Alpha is self-hosting.

It is grown the same way the on-ramp was — smallest useful thing first, then widen
the accepted grammar slice by slice until it matches what `alpha-rs` accepts. The
on-ramp is the executable spec; this must accept the same language.

## Build / test

The compiler is a filter: source on stdin, a Windows PE on stdout.

```
../alpha-rs/target/debug/alpha.exe alpha.alp alpha0.exe   # on-ramp compiles it
printf '42' | ./alpha0.exe > out.exe                       # it compiles "42"
./out.exe; echo $?                                             # -> 42
```

## Status (incremental)

- **Increment 1 — PE emitter + `mov eax,N; ret` stub: DONE.** Proved the Alpha→PE
  pipeline and built the byte-exact PE writer (`write_pe` + `emit_u16/u32/zeros`),
  transcribed from `alpha-rs/src/pe.rs` (single-section path).
- **Increment 2 — lexer: DONE.** `read_source` reads stdin into a `[u8;65536]`
  buffer; `lex` tokenizes it into parallel `tok_kind/tok_start/tok_len` arrays
  (`[i32;16384]`) with a `tok_count`. Handles idents, ints, string literals,
  `//` comments, whitespace, the two-char ops (`-> :: == != <= >=`) and all
  single-char punctuation. To verify end to end, `main` emits a PE that exits with
  the token count. Confirmed against 8 snippets (e.g. `self.n = self.n + 1;` → 11
  tokens incl. Eof). The big token arrays live in the static `.data` section (the
  enabler), so the compiler exe is ~470 KB. No on-ramp gaps surfaced.

- **Increment 3 — parse + emit `exit_process(<int>)`: DONE.** `tok_int` parses a
  decimal value from a token's source bytes; `parse` walks the token stream and
  takes the first integer literal as the exit code; `main` emits the `mov eax,N;
  ret` entry stub and writes the PE. So `alpha` now does a real translation:
  source → tokens → value → code → PE. Verified: `machine main { exit_process(42) }`
  → 42 (and 7/200/123), and it compiles the on-ramp's own `samples/exit7.alp` → 7.
  (Still a stepping stone — "first int wins", no keyword matching yet.)

- **Increment 4 — arithmetic expressions: DONE.** Keyword matching (`is_exit_process`
  byte-compares the token) + an **iterative shunting-yard** expression compiler
  (`compile_expr`) that emits the x64 stack-machine code for `+ - * /` with correct
  precedence/associativity and trap-on-overflow — no recursion (an explicit
  operator stack), since Alpha bans call recursion. `emit_code`/`emit_push_imm`/
  `emit_binop` build into `self.code`. Verified: `3 + 4 * 2`→11, `2 * 3 + 4`→10,
  `20 - 3 - 2`→15, `100 / 5 / 2`→10. No on-ramp gaps.

- **Increment 5 — `let`/locals: DONE.** A frame prologue (`push rbp; mov rbp,rsp;
  sub rsp,512`), a symbol table (`local_tok[]` + `tok_eq` ident byte-compare), and
  `let IDENT: TYPE = expr;` storing into an rbp slot; identifiers in expressions
  load from their slot. `find_machine` skips `boundary`/`data` blocks by tracking
  brace depth and matching the **top-level** `machine` keyword. `emit_i32` writes
  correct two's-complement bytes for negative rbp displacements via `ashr8` (Alpha's
  `/` truncates, so a floor-division bias gives the arithmetic shift). Verified:
  two-local programs, `y = x*2`, and it compiles the on-ramp's own `exit7.alp`→7,
  `arith.alp`→11, `locals.alp`→14. No on-ramp gaps (one bug, in alpha's own
  depth-blind machine-finding, found + fixed).

- **Increment 6 — comparisons + `transition`/`state` control flow: DONE.**
  Comparison operators (`< > <= >= == !=`, precedence 0) emit `cmp`/`setcc`.
  States are pre-scanned (depth-aware) into a name table and become code labels;
  a `transition` compiles its subject into eax then emits `cmp eax,imm; je` per
  int/`true`/`false` arm and `jmp` for `_`, recording forward-jump fixups that are
  patched once every state's offset is known. Local reassignment `x = e;` added.
  The keyword matchers were unified into one `keyword_equals`. Verified: a counter
  loop (0→3), multi-arm dispatch (x=2→22, x=5→99), a sum-1..=5 loop (→15); samples
  still compile (exit7/arith/locals). No on-ramp gaps.

- **Increment 7a — multi-machine + free calls + `return`: DONE.** A program is now
  a list of machines: `prescan_machines` records each top-level machine's start +
  callable name (and finds the entry `main`); `parse` emits the entry first (PE
  entry stays at offset 0) then the rest, recording cross-machine `call rel32`
  fixups that `patch_calls` resolves. `emit_machine` parses a header (spilling value
  params from rcx/rdx/r8/r9 to frame slots) and a `return` statement. `compile_call`
  evaluates args, loads the first four into registers, `call rel32`s the callee
  (recursion-safe via per-call `let` locals + a base-relative operator stack).
  Verified: `add(20,22)`→42, nested `inc(inc(inc(5)))`→8, 4-arg `sum4`→10, and
  forward references in any source order. (Recursion is permitted for now, as in the
  on-ramp; DAG-purity is deferred.)

- **Increment 7b — `data` + `self` fields + methods: DONE.** `prescan_data` lays
  out the data type's fields (scalar 8B, capability 0B, arrays N×width). The entry
  machine's `self` is a zero-init `.data` section reached via a self-pointer
  (`lea rax,[rip+data]` + a patched RIP-relative reloc; methods get self in rcx).
  `self.field` reads/writes go through it; `self.m(args)` method calls pass self in
  rcx and args in rdx/r8/r9. `write_pe` now emits a two-section PE (`.text` +
  writable `.data`) when there's data. Verified: mutable self fields (10/34),
  a counter held in `self` across a loop (5), `bump`×3 (3), `add_to(5)` (15);
  no-data programs still emit a single-section PE.

- **Increment 7c — array fields + trap-checked indexing: DONE.** `self.arr[i]`
  reads (`movzx`/`mov` byte or dword) and `self.arr[i] = e` writes compute
  `self_ptr + field_offset + i*width` after a `cmp i,N; jb +2; ud2` bounds check —
  an out-of-bounds index traps (SIGILL). `emit_self_element_addr` shares the address
  math; the index is any expression (`compile_expr` now also flushes at `]`).
  `prescan_data` records each array field's element count for the bounds check, and
  `resolve_field` reports the resolved field's width + count alongside its offset.
  The field layout is saved to frame locals before the index/value sub-expressions
  so a nested `resolve_field` can't clobber it (this also fixed a latent 7b bug
  where `self.a = self.b + 1` wrote to `b`'s offset). Verified: i32 array (15),
  loop fill+sum with a runtime index (30), u8 buffer (74), OOB → trap (exit 132).

- **Increment 7d — byte I/O + the import-table PE: DONE.** `write_byte(b)` and
  `read_byte()` lower to `GetStdHandle`+`WriteFile`/`ReadFile` (Win64 ABI: the args
  in rcx/rdx/r8/r9 + the 5th at `[rsp+0x20]`, I/O scratch in fixed frame slots);
  `read_byte` returns the byte or -1 at EOF (branchless `cmove`). `write_pe` grew to
  emit a `.rdata` section with a full kernel32 import table (IDT/ILT/IAT + hint/name
  + dll name) and patch the `call [rip+IAT]` relocations, and the optional header's
  import + IAT data directories. `.data` became a BSS section (the loader zero-fills,
  so a multi-MB instance costs no file bytes). The frame grew to 640 bytes for the
  I/O scratch above the call's shadow space.

## ✅ SELF-HOSTING REACHED

`alpha.alp` compiles itself to a byte-identical fixed point:

```
alpha-rs (on-ramp)  --compiles-->  alpha0      (throwaway-built)
alpha0             --compiles-->  alpha1      (first Alpha-built)
alpha1             --compiles-->  alpha2      (alpha1 == alpha2, byte-identical)
alpha2             --compiles-->  alpha3      (== alpha2)
```

alpha1 == alpha2 == alpha3 (same MD5), and the Alpha-built compiler compiles the
full language (arithmetic + precedence + parens, control flow + loops, a DAG of
machines with params/returns, `data` + mutable `self` fields, scalar/byte arrays with
trapping indexing, and byte stdin/stdout I/O). The on-ramp (`../alpha-rs`) is now
discardable — Alpha builds Alpha.

The bug that blocked convergence: `compile_expr` ignored parentheses (it flushed at
`)`), so the on-ramp-built `alpha0` correctly ran `align_up`'s `((v+a-1)/a)*a` while
`alpha1` (built by `alpha0`, which couldn't parse the parens) miscomputed every PE
size by `+code_length`. Adding shunting-yard paren handling (push a `(` marker, pop to
it on `)`, with `(` ranked below every operator) closed the fixed point.

### Rebuild + verify the fixed point

```
../alpha-rs/target/debug/alpha.exe alpha.alp alpha0.exe
./alpha0.exe < alpha.alp > alpha1.exe
./alpha1.exe < alpha.alp > alpha2.exe
cmp alpha1.exe alpha2.exe && echo "self-hosting holds"
```

## Known gaps to fix in the on-ramp as they surface

- Large `self`/frames (a real source buffer is tens of KB) will exceed one stack
  page — needs stack probing in the prologue, or a static instance. Increment 1
  dodges this with a small buffer.
