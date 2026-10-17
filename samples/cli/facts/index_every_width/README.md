# Index Every Width

Compile-scope probe: every integer width indexes builtin array
geometry. No standard-library dependency.

- `u8 [0..=3]` through `i64 [0..=3]` bounded scalar parameters —
  signed indices carry their own nonnegative range; `^` folds the
  reads.
- `read_slice(&[u64], position: i32 [0..=3]) requires position <
  values.len` — signed bound + requires roster on a slice view.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/facts/index_every_width/main.omg
mbx run -p omega -- update --project samples/cli/facts/index_every_width
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
