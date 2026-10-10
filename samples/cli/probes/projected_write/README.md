# Projected Write

Depend-free compile-scope probe: write-only projected receivers — the
first sample passing `&write` borrows aimed inside an aggregate through
calls.

- `forward(&mut main.pair, -43)` takes a mutable receiver and projects
  it: `&write pair.right` addresses the right Cell directly — a
  write-only borrow of a field-of-field.
- The projected `&write cell` forwards through two relay states and lands
  in `Cell::replace(cell: &write Cell, ...)` — each hop keeps the
  write-only capability; reads of the borrowed cell never occur.
- `main.pair.left` is written directly through `main` — the mutable
  entry carrier — and left untouched by the projection.

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/projected_write/main.omg
mbx run -p omega -- update --project samples/cli/probes/projected_write
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
