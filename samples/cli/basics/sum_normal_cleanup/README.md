# Selected sum cleanup on normal return

Pass empty and populated sums to an ordinary scalar-returning machine without
an authored case dispatch. Its normal exit must dispose only the active payload,
then return the captured scalar. Correct execution returns 0.

```text
omega update --project samples/cli/basics/sum_normal_cleanup
omega run --both samples/cli/basics/sum_normal_cleanup/main.omg
```

This valid program challenges the shared cleanup-control boundary for
**BORROWED-STORAGE-RESTORATION**; it is not an expected-rejection fixture.
