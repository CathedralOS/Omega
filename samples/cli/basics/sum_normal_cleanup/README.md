# Selected sum cleanup on normal return

Pass empty and populated sums to an ordinary scalar-returning machine without
an authored case dispatch. Its normal exit must dispose only the active payload,
then return the captured scalar. Correct execution returns 0.

The parent hook changes a marker after its return operand is captured; selected
child cleanup must retain the parent's home without changing that result.
Projected return and state-transfer operands leave their old owner before its
runtime-selected remainder is disposed. Their observed values stay 23 and 29.
Borrowed computed-call temporaries select their cleanup after the call completes;
both empty and populated producer results preserve the captured answer 31.

```text
omega update --project samples/cli/basics/sum_normal_cleanup
omega run --both samples/cli/basics/sum_normal_cleanup/main.omg
```

This valid program challenges the shared cleanup-control boundary for
**BORROWED-STORAGE-RESTORATION**; it is not an expected-rejection fixture.
