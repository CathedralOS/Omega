# Identity measure with constrained signature

This legacy fixture still declares bounded `u64` parameter/result types using
the scalar range-annotation suffix selected for removal. Its parameter bound
must become `requires` and its result bound must become `ensures` under
`CANONICALIZE-SCALAR-RANGE-CONTRACTS` on the [board](../../../../../TASKS.md).
A named domain is appropriate only if this fixture independently needs that
nominal contract; this README does not claim the migration is implemented.

The existing refinement is the view's domain contract: it applies only when the
ranked subject's enforced bounds fit inside every declared range. Here `remaining`
carries exactly `0..=5`, so the identity forward is admissible and the range
`0..=5` contains the produced rank.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/constrained_measure_parameter_compile/main.omg
```

The [domain control](../../../fail/termination/constrained_measure_parameter_domain/main.omg)
narrows the declared domain to `1..=5`, which the subject's `0..=5` bounds do not satisfy, and must reject.
