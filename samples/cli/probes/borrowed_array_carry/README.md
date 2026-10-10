# Borrowed Array Carry

Compile-scope probe: `&mut [u8; 17]` borrowed closed arrays across
state re-entry edges. No standard-library dependency.

- `write_bytes(out: &mut [u8; 17], ...)` — a sized-array borrow
  parameter (not a slice): the referent stays caller-owned while the
  graph forwards the same borrow on each loop edge.
- `out[position] = value` through the borrow; `out.len` known at
  every edge.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/borrowed_array_carry/main.omg
mbx run -p omega -- update --project samples/cli/probes/borrowed_array_carry
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
