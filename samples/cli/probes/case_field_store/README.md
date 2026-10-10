# Case Field Store

Compile-scope probe: case-field stores on non-parameter roots. No
standard-library dependency.

- `boxed.fmt = fresh` where `boxed` is an owned local — the
  argument flows into a field of a locally-constructed record.
- `machine Main::make -> Point` — record literal as an arm result
  after a side-effecting body (`main.calls += 1` counted once).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/case_field_store/main.omg
mbx run -p omega -- update --project samples/cli/probes/case_field_store
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
