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
../alpha-rs/target/debug/alphac.exe alphac.alp alphac0.exe   # on-ramp compiles it
printf '42' | ./alphac0.exe > out.exe                          # it compiles "42"
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
  ret` entry stub and writes the PE. So `alphac` now does a real translation:
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
  `arith.alp`→11, `locals.alp`→14. No on-ramp gaps (one bug, in alphac's own
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

## Next increments

7. Machine calls, `data`/arrays, byte I/O, the import-table PE path — the full
   on-ramp language. Hand-specialize arenas (no generics). Then close the fixed point.

## Known gaps to fix in the on-ramp as they surface

- Large `self`/frames (a real source buffer is tens of KB) will exceed one stack
  page — needs stack probing in the prologue, or a static instance. Increment 1
  dodges this with a small buffer.
