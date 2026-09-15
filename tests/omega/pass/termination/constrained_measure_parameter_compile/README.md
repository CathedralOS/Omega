# Identity measure with constrained signature

`Countdown::Remaining` declares its parameter and result over `u64 [0..=5]`.
The refinement is the view's domain contract: it applies only when the ranked
subject's enforced bounds fit inside every declared range. Here `remaining`
carries exactly `0..=5`, so the identity forward is admissible and the range
`0..=5` contains the produced rank.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/constrained_measure_parameter_compile/main.omg
```

The [domain control](../../../fail/termination/constrained_measure_parameter_domain/main.omg)
narrows the declared domain to `1..=5`, which the subject's `0..=5` bounds do not satisfy, and must reject.
