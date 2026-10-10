# Write-Only Forward

Compile-scope probe: `&write` write-only projected borrows. No
standard-library dependency.

- `replace(destination: &write i32, value)` — the callee assigns
  `destination = value` with no deref spelling and can never read
  the pointee.
- A `&write` parameter forwards by re-projection:
  `&write destination` passes the same write loan deeper through a
  state hop.
- `neighbor` untouched — the loan sees only its target.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/write_only_forward/main.omg
mbx run -p omega -- update --project samples/cli/probes/write_only_forward
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
