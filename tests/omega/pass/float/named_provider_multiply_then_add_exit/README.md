# Named multiply-then-add

From the repository root, settle the local standard-library dependency and run:

```sh
python3 tools/benchmark/benchmark.py prepare --root tests/omega/pass/float/named_provider_multiply_then_add_exit/main.omg --target macos_arm64 --omega target/release/omega
target/release/omega run --both tests/omega/pass/float/named_provider_multiply_then_add_exit/main.omg
```

Expected native exit: **70**. In PowerShell use `python` and the built
`target/release/omega.exe`, selecting `windows_x86_64` during preparation.

Binary32 and binary64 cancellation distinguish two rounded operations from
fused multiply-add's nonzero residual. The binary32 result is consumed by a
surrounding addition, while binary64 and overflow results are stored directly.
The named core requirement has unqualified operands and result: explicit policy
erasure before its overflow call yields infinity, not saturation. This follows
the [numeric contract](../../../../../wiki/spec/language/numeric_values.md#floating-formats-and-operations)
and `source/library/core/float_operations.omg`; the separate
`float_saturating_arithmetic_exit` fixture retains policy-adapter coverage.

The checked interpreter currently declines the selected bodyless float provider;
`--both` must report that limitation rather than claim interpreter agreement.
