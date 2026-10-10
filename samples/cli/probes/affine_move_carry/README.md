# Affine Move Carry

Depend-free compile-scope probe: affine values — the first sample where
plain data (no `[copy]` marker) moves through calls and cycles exactly
once.

- `route` moves `ticket` into `take` on the true arm only; the false arm
  leaves the binding untouched and hands `take` a fresh Ticket — each
  arm consumes exactly one value.
- `loop_until` carries `ticket` through the `spin` cycle on every
  iteration and delivers it to `finish` on exit — the affine value stays
  live across the loop without copying.
- `direct` reads a moved-into-call result (70) and `diverted` reads the
  untouched arm's fresh construction (71).

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/affine_move_carry/main.omg
mbx run -p omega -- update --project samples/cli/probes/affine_move_carry
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
