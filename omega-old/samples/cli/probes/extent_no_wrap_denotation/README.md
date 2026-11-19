# extent_no_wrap_denotation

First sample using `no_wrap(base, length)` — the
`omega::language::core::extent` address-arithmetic
denotation — as a machine `requires`, plus the
caller-requires forwarding that discharges the callee's
clause.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Gaps found while authoring (filed)

- `no_wrap` is a proof-only mathematical term: it cannot
  be a transition subject ("mathematical terms are
  proof-only and cannot be used as executable values or
  statements") and literal call arguments cannot reach it
  (`no_wrap(4096, 512)` fails a mathematical-application
  type check — `addr` expected, `u64` literal given). Only
  a caller's own requires or a domain/prior guarantee
  discharges the callee's `no_wrap` clause — no
  value-level guard or literal establishment exists.
