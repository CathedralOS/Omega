# Saturating field guarantee across a Unit call

`Countdown::arm(4)` stores `5` and guarantees that the field is positive and
that its Saturating subtraction by one is below eight. `tick` consumes those
guarantees, stores `4`, and returns it. The authored exit is `70`; a wrong value
exits `71`.

From the repository root after building `omega`:

```sh
omega update --project tests/omega/pass/contracts/ensures_saturating_field_arithmetic_exit --target linux_x86_64
# Review the generated local package decisions and accept them in the review file.
omega update --resume --project tests/omega/pass/contracts/ensures_saturating_field_arithmetic_exit
omega run --both tests/omega/pass/contracts/ensures_saturating_field_arithmetic_exit/main.omg
```

Before the repair, checked source passed but Terminal production rejected the
exit guarantee with `OperationProofUnavailable`. Inclusive bounds now replay
through clamped addition and subtraction using the shared affine certificate.
Saturation plateaus do not imply strict order or an inverse translation.
