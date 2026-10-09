# Boundary suspension with inline completion

Run `omega run --both tests/omega/pass/concurrency/boundary_suspension_inline_completion_exit/main.omg`.
Native and Terminal execution must exit 0 after observing the returned 42,
the retained scalar parameter 40, its local alias and the retained local 19.
The authored state has a structural parameter before its scalar parameter,
and a structural local before its scalar local: omitted storage must not shift
source positions.

The source-selected checked provider completes inline. The requirement still
declares possible suspension, so lowering must retain its exact requirement
identity and checked live bindings rather than rejecting every such call. This is
not evidence of parking, concurrency, cancellation or task settlement.
