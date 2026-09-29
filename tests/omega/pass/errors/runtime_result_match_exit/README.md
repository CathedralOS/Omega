# Result construction and dispatch in a loop

From the repository root, after preparing this checkout's local package review:

```sh
omega run --both tests/omega/pass/errors/runtime_result_match_exit/main.omg
```

Both engines must exit 70. The loop computes `10 / 2`, handles `7 / 0` as
error data without dividing, and computes `20 / 4`: two successes totaling
10 and one error. Each result replaces the previous affine field value;
case payloads then flow into the selected handler state.

The receiver syntax is existing `STATIC-MACHINE-MIGRATION` debt, not a second
language contract. This regression retains its authored algorithm and guards.
