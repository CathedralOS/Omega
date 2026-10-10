# Const Sized Buffer

Depend-free compile-scope probe: compile-time machine evaluation — the
first sample with a machine call in array-length position.

- `triangle` is a measured tail-recursive free machine; its `requires`
  roster is the arrival invariant (independent bounds on `n` and `acc`
  would admit a violating call), `terminates by n` proves the loop.
- `table_size()` const-evaluates `triangle(4, 0) == 10`, and the call
  sits in `[u8; table_size()]` — the array's length IS the machine's
  compile-time result.
- `Ledger {}` omits the array field: zero-fill supplies it, so
  `ledger.buf[9] = 7` is the first legal write to the last slot.

The machine writes slot 9, reads it back, and accepts on 7 && 10.
Compile-checked only (no std dependency):

```sh
mbx run -p omega -- --check samples/cli/probes/const_sized_buffer/main.omg
mbx run -p omega -- update --project samples/cli/probes/const_sized_buffer
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
