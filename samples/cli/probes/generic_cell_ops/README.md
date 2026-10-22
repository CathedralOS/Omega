# Generic Cell Ops

Historical implementation probe. Data-qualified generic machines are retired
under `no-data-code-attachment`; their bodies and type parameters must migrate
to ordinary free/module declarations under `STATIC-MACHINE-MIGRATION` in
[TASKS.md](../../../../TASKS.md). The compile observations below are not evidence
that the old spelling remains a supported language form.

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
