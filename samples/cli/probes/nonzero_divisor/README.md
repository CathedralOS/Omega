# Non-Zero Divisor Domain

Compile-scope probe: a case payload carrying a predicate domain. No
standard-library dependency.

- `domain i32::NonZero(value) requires value != 0` — a predicate
  domain, not an interval: `NonZero` excludes one value from the
  middle of i32, so no bounded carrier represents it.
- `Divisor::By(amount: i32 in NonZero)` — construction proves `7 != 0`.
- `Divisor::By { amount }` — record-name case-payload destructure; the
  dispatched payload carries the membership into `490 / amount`,
  discharging the divisor obligation with no guard.
- `hits` proves the quotient arm ran.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/nonzero_divisor/main.omg
mbx run -p omega -- update --project samples/cli/probes/nonzero_divisor
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
