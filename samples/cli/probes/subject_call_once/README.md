# Subject Call Once

Compile-scope probe: a transition's guard subject evaluates exactly
once per evaluation. No standard-library dependency.

- `transition Main::roll(&mut main.tally)` — call subject with a
  mutable borrow; both arms compare the same evaluation.
- `Main::roll -> bool` counts its invocations through `&mut Tally`
  and returns false; the second arm proves `calls == 41` — one
  execution, not one per arm.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/subject_call_once/main.omg
mbx run -p omega -- update --project samples/cli/probes/subject_call_once
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
