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
../alpha-rs/target/debug/alphac.exe alphac.alpha alphac0.exe   # on-ramp compiles it
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

## Next increments

3. Parser (tokens → an AST in arenas) + the x64 stack-machine emitter (port of
   `alpha-rs/src/x64.rs`), starting with `machine main { exit_process(N) }`.
4. Widen to expressions, locals, control flow, calls, data/arrays — the full
   on-ramp language. De-recurse tree-walks to an explicit worklist (Alpha bans call
   recursion); hand-specialize arenas (no generics). Then close the fixed point.

## Known gaps to fix in the on-ramp as they surface

- Large `self`/frames (a real source buffer is tens of KB) will exceed one stack
  page — needs stack probing in the prologue, or a static instance. Increment 1
  dodges this with a small buffer.
