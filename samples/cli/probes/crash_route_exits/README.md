# Crash Route Exits

Depend-free compile-scope probe: explicit crash routes — the first sample
declaring `crashes Trap` on ordinary machines.

- `inspect` declares its unrecoverable route: the `n < 0` arm reaches
  `crash Trap;` — an unwind-free terminal outcome — while the other arm
  returns true.
- `probe` covers the callee's route by declaration: `crashes Trap` on its
  own signature propagates `inspect`'s route without discharging it.
- The entry likewise declares `crashes Trap` and calls `probe(5)` on the
  dormant path — every route exists in the checked program while none
  fires in this run.

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/crash_route_exits/main.omg
mbx run -p omega -- update --project samples/cli/probes/crash_route_exits
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
