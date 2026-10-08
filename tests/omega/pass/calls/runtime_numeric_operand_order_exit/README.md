# Numeric operand evaluation

From the repository root:

```sh
omega run --both tests/omega/pass/calls/runtime_numeric_operand_order_exit/main.omg
```

Native and Terminal execution must both exit 0. The program checks nine helper
calls, left-to-right effects, single evaluation, skipped short-circuit operands,
and a scalar observation captured before a later operand mutates its backing.
`abs` and `clamp` normalize into scalar computations; their nested integer
`min`/`max` calls must not become extra machine invocations or unknown storage
writes. Authored same-spelled machines keep their ordinary call contracts.
