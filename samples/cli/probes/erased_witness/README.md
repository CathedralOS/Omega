# Erased Witness

Compile-scope probe: `[erased]` proof-side positions. No
standard-library dependency.

- `case Proven(value: i32, proof [erased]: Evidence)` — the erased
  payload fills itself from its unique nullary constructor at
  construction; destructuring discards it `proof as _`.
- `bound [erased]: i32` — an erased machine parameter used only by
  `requires n < bound`, forwarded to a state's own erased parameter
  as that binding's initializer.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/erased_witness/main.omg
mbx run -p omega -- update --project samples/cli/probes/erased_witness
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
