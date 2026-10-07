# Stored constructor observations

`omega run --both tests/omega/pass/calls/runtime_stored_constructor_snapshots_exit/main.omg`
exits 0 natively and in Terminal execution. The first field copies an unknown
incoming value before the mutable call; the second copies the value established
by that call's declared postcondition. Copying the record through assignment
must retain both observations. The compiler must not replay its initializer or
use the constructor's final context for its earlier children.
