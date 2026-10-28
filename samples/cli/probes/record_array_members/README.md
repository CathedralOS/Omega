# record array members

Compile-scope probe. No standard-library dependency.

- numeric_policies: `narrow_<S>_to_<T>_{exact,wrapping,saturating,
  trapping}` + `widen_*` from core::numeric_conversion — policy
  in the machine name.
- numbered_cases: `#1 case Found(#1 value: T, retired #2)`,
  `retired #3` — numbered case/member identities.
- record_array_members: `[i32;4]` and `[[i32;2];2]` members in
  record literals + negative operands `0 - 2` inside array
  literals.
- borrowed_operator: `machine + Wrapped::add(left: &Wrapped,
  right: &Wrapped) -> u64` — declared operator on borrows,
  result type independent of operands; named call; match-arm
  operator result; `2^64` anonymous comptime literal.

Numbered-case source migration is tracked by `PREFIX-CASE-IDENTITIES` in
[TASKS.md](../../../../TASKS.md); the canonical spelling above is not a fresh
compile result for this legacy probe.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/record_array_members/main.omg
mbx run -p omega -- update --project samples/cli/probes/record_array_members
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
