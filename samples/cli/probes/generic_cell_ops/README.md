# Generic Cell Ops

Compile-scope probe: static generic attached machines on generic
data. No standard-library dependency.

- `data Cell<T> { value: T; touched: i32 in Wrapping }` — generic
  payload + independent domain field.
- `Cell::set<T>` / `Cell::touch_count<T>` — unbounded generic
  attached machines.
- `Cell::get<T [copy]>` — the getter alone needs a copy bound:
  returning the field through a shared borrow copies it; container
  and setter need none.
- `transition { _ -> (cell.value) }` — bare arm returning a field.
- Two instantiations (i32, bool) coexist.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/generic_cell_ops/main.omg
mbx run -p omega -- update --project samples/cli/probes/generic_cell_ops
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
