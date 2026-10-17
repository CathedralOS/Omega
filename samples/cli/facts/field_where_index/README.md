# Field `where` Facts + Computed Index

Compile-scope probe: a data `where` roster carrying standing field
facts. No standard-library dependency.

- `data Main where 0 <= k, k <= 3 { ... }` — the bound holds for every
  `main` value at every point.
- `main.arr[main.k + 1]` — the computed index is discharged by the
  standing bound alone (no guard), in the entry AND a later state —
  the fact survives transitions.
- `transition { _ -> successor(main) }` — bodyless transition =
  unconditional goto.
- `[i32 in Wrapping; 5]` — element-domain array field.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/facts/field_where_index/main.omg
mbx run -p omega -- update --project samples/cli/facts/field_where_index
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
