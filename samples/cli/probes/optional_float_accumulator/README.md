# Optional float accumulator

Depend-free compile-scope probe: an `Optional<f64>` presence-case dispatch
inside an f64 accumulation loop. It is the first sample to exercise
`Optional` as user data (its only prior uses are internal carriers inside
the std packages) and to run a float accumulator across a state-machine
counter without the standard-library package.

`term(n)` returns `Optional::Some` for `n` in 1..=8 — the first eight
reciprocals — and `Optional::None` afterwards. The `fold` state reads each
term through a case dispatch on `Optional`, and `step` applies the
dominating `n < 8` guard so `n + 1` stays provably in range (exact
arithmetic is a proof obligation). `judge` accepts a harmonic partial sum
in `(2.71, 2.72)`; the true value is `H8 = 2.717857…`.

With no standard-library dependency there is no `ProcessExit` provider,
so this is compile-checked only:

```sh
mbx run -p omega -- --check samples/cli/probes/optional_float_accumulator/main.omg
mbx run -p omega -- update --project samples/cli/probes/optional_float_accumulator
```

Both complete green on Linux x86-64; the package review publishes
`omega.lock` locally (ignored, like every sample lock).
