# Named multiply-then-add

From the repository root, run the focused native/Terminal qualification:

```sh
mbx xtask qualify --runtime pass/float/named_provider_multiply_then_add_exit --output build/qualify-multiply-then-add
```

Expected native and Terminal exit: **70**.

Binary32 and binary64 cancellation distinguish two rounded operations from
fused multiply-add's nonzero residual. The binary32 result is consumed by a
surrounding addition, while binary64 and overflow results are stored directly.
The named core requirement has unqualified operands and result: explicit policy
erasure before its overflow call yields infinity, not saturation. This follows
the [numeric contract](../../../../../wiki/spec/language/numeric_values.md#floating-formats-and-operations)
and `source/library/core/float_operations.omg`; the separate
`float_saturating_arithmetic_exit` fixture retains policy-adapter coverage.

Both native execution and Terminal Psi must return 70. The target table selects
the checked multiply-then-add body under the same two-rounding contract.
