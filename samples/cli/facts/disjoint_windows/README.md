# disjoint windows

Compile-scope probe. No standard-library dependency.

- `items[0..2]` and `items[2..4]` — literal disjoint bounds let two
  `&mut` subslices of one base borrow at once; writes through both.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/facts/disjoint_windows/main.omg
mbx run -p omega -- update --project samples/cli/facts/disjoint_windows
```
