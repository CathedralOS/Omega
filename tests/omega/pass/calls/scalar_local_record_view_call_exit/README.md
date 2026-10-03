# Scalar calls through a record's borrowed view

From the repository root:

```sh
omega run --both tests/omega/pass/calls/scalar_local_record_view_call_exit/main.omg
```

The scalar `run` helper constructs a record containing the interior window
`backing[1..3]`, lends that record to `inspect`, and forwards its view field to
`inspect_values`. The final state checks both the window's shortened extent and
its elements. Correct execution exits 70; a wrong observation exits 71.

This covers established local storage, projected descriptor arguments, scalar
call results and exact cleanup of the record at scalar return. The borrowed
backing remains owned by `run` throughout the calls.
