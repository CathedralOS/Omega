# Shared numeric operand evaluation

From the repository root:

```sh
omega run --both tests/omega/pass/calls/runtime_numeric_operand_single_evaluation_exit/main.omg
omega run --both tests/omega/pass/calls/runtime_numeric_operand_order_exit/main.omg
```

The first program increments a counter inside `abs(step(...) + 1)`. The
normalized comparison and negation must share that one evaluated operand:
the result is 1, the counter is 1, and native and checked execution exit 0.

The second program distinguishes shared operands from separately authored
calls, checks left-to-right effects, returns a normalized intrinsic from a
helper twice, leaves a short-circuit operand unevaluated and snapshots a field
before a sibling call changes it. Its counter is 9 and both executions exit 0.
No snapshot survives an outer expression or
callee invocation. The numeric-operand rejection fixtures retain independent
callee-precondition, exclusive-access and exact-arithmetic checks.
