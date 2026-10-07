Two pure identity calls with the same unknown scalar input construct equal
Pair fields. The source requirement is valid; both execution engines must
finish with exit 0.

After package preparation, run:

```text
omega run --both tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg
```

The public command exits 0 in native and Terminal execution. Psi discharges the
call obligation over exact pure-call results. The former independent Terminal
CallRequires certificate is removed by the PCC ruling; it is not an unfinished
compiler capability. The fixture remains without an authored ensures clause.

Keep `pending/calls/constructor_child_local_epochs_not_equal` as a separate
source-checking defect: reads separated by a write cannot share a value identity.
