# Generic data + match

Depend-free compile-scope probe: user-declared generic data and `match`
expressions — the first sample to declare a multi-parameter `data`, to
instantiate a generic machine across two type parameters, and to branch
with `match` producing fresh enum/record values.

- `Pair<A, B>` is ordinary generic data; field reads on a concrete
  instance erase to the selected types.
- `swap<A, B>` is a generic machine specialized per call site.
- `match` yields values (enum constructors in the arms).
- transitions on an enum value destructure its payload case.

The machine swaps `Pair<20u64, 1i32>` into `Pair<1i32, 20u64>`, classifies
code 7 to `Verdict::Fresh(7)`, and accepts only when the swapped fields
and the fresh payload agree. Compile-checked only (no std dependency, no
ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/generic_data_match/main.omg
mbx run -p omega -- update --project samples/cli/probes/generic_data_match
```

Both complete green on Linux x86-64; `omega.lock` publishes locally and is
ignored like every sample lock.
