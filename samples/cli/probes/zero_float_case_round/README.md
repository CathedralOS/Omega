# Zero-constructed float case, half away from zero

A `[copy]` record whose first case carries `f64` lives in a length-2 array.
Writing only `ready` leaves the case at its zero value: all-zero bytes are
`+0.0`, so both slots are `Reading::Value { amount: +0.0 }`. The program
rounds that payload and two runtime-indexed ratios, `-1/2` and `1/2`, half
away from zero. The second slot's `ready` flag is then cleared and must not
disturb its float payload.

```sh
omega update --project samples/cli/numeric/zero_float_case_round --target linux_x86_64
omega run samples/cli/numeric/zero_float_case_round/main.omg < /dev/null
```

Expected exit 0. Expected output contains `zero float case rounds -1 and 1`.

`invalid/` is rejected because index `2` is outside a length-2 `f64` array.
