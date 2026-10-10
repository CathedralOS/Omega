# Indexed Storage

Depend-free compile-scope probe: fixed arrays through the carrier — the
first sample reading and writing `[i32; N]` by index.

- `data Main { cells: [i32; 3]; }` holds the array on the entry carrier;
  `main.cells = [10, 20, 20]` assigns an array literal and
  `main.cells[1] = 40` rewrites a single element in place.
- `main.cells[i]` reads by index; the `as i32 in Wrapping` casts opt the
  tally into a defined-overflow domain (exact `i32` addition is a proof
  obligation — decision 17).
- The guard accepts only the rewritten sum: 10 + 40 + 20 = 70 — pinning
  both the literal assignment and the per-element write.

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/indexed_storage/main.omg
mbx run -p omega -- update --project samples/cli/probes/indexed_storage
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
