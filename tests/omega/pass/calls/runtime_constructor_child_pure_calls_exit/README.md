Two pure identity calls with the same unknown scalar input construct equal
Pair fields. The source requirement is valid; both execution engines must
finish with exit 0.

After package preparation, run:

```text
omega run --both tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg
```

The public command exits 0 in native and Terminal execution. Psi discharges the
call obligation over exact pure-call results; lowering consumes the retained
source judgment. The fixture has no authored ensures clause.

Keep `pending/calls/constructor_child_local_epochs_not_equal` as a separate
source-checking defect: reads separated by a write cannot share a value identity.
