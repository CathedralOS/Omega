The independently selected `Reader` calls Darwin `_abs(-42)` through an evaluated
foreign binding. The consumer traps unless it receives 42. Cold and warm native
execution must agree with Terminal Psi; the warm invocation must restore the
complete component, including the evaluated binding identity namespace.

This macOS witness covers native-product persistence under `CHECKED-PACKAGE-REUSE`.
It does not establish persisted checked-package graphs, other-host qualification,
or the parent task's timing acceptance. Callback-bearing imports remain outside
this execution evidence: their separate baseline witness refuses before native
production.
