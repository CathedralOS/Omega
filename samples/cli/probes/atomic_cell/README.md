# Atomic Cell

Compile-scope probe: `AtomicU32` value-method operations. No
standard-library dependency.

- `main.counter.store(10, NoOrdering)` / `fetch_add(5, NoOrdering)`
  / `compare_exchange(15, 99, ReceivePublish, Receive)` — each
  spells its ordering; `fetch_add` and `compare_exchange` return
  the observed PRIOR value.
- `main.counter == 15` — the cell compares directly against a
  scalar.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/atomic_cell/main.omg
mbx run -p omega -- update --project samples/cli/probes/atomic_cell
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
