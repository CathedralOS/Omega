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

- **Increment 1 — PE emitter + minimal front-end: in progress.** Accepted "grammar"
  is a single decimal integer N; output is a PE whose entry is `mov eax,N; ret`, so
  it exits with N. This exists to (a) prove the Alpha→PE pipeline end to end and
  (b) build the byte-exact PE writer (`write_pe`) + byte-emit helpers that the full
  compiler reuses. Code lives in a `[u8;N]` self buffer; the PE structure is
  transcribed from `alpha-rs/src/pe.rs` (single-section, no-imports path).

## Next increments

2. A real lexer + `exit_process(N)` grammar (tokens, a tiny parser).
3. Expressions, locals, the x64 stack-machine emitter (port of `alpha-rs/src/x64.rs`).
4. Control flow, calls, data/arrays — widen to the full on-ramp language.
5. De-recurse any tree-walks to an explicit worklist (Alpha bans call recursion);
   hand-specialize arenas (no generics). Then close the self-hosting fixed point.

## Known gaps to fix in the on-ramp as they surface

- Large `self`/frames (a real source buffer is tens of KB) will exceed one stack
  page — needs stack probing in the prologue, or a static instance. Increment 1
  dodges this with a small buffer.
