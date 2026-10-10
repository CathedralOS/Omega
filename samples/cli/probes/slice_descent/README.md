# Slice Descent

Compile-scope probe: slice-length termination measures. No
standard-library dependency.

- `machine Main::walk(...) terminates by entries` — default order
  over the slice's length; each call forwards `entries[1..]`.
- Runtime recursion is TAIL-ONLY: the measured machine carries its
  count in a `depth` accumulator — `walk(...) + 1` feeding an
  operand is rejected as non-tail self-recursion (measured recursion
  amendment).
- `&main.entries[..]` — whole-array shared slice view; the `&mut`
  entry borrow weakens to `&` for the callee.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/slice_descent/main.omg
mbx run -p omega -- update --project samples/cli/probes/slice_descent
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
