# Boundary suspension with inline completion

Run `omega run --both tests/omega/pass/concurrency/boundary_suspension_inline_completion_exit/main.omg`.
Native and Terminal execution must exit 0 after observing the returned 42,
the mutable scalar parameter changed from 39 to 40, its local alias and the retained local 19,
the structural marker 19 and the retained linear inventory 23. The provider
consumes a different linear inventory forwarded through an ordinary call and
the structural argument lane;
the caller then consumes its retained inventory exactly once.
The authored state has a structural parameter before its scalar parameter,
and a structural local before its scalar local: omitted storage must not shift
source positions. Mixed scalar/structural actuals retain the original checked
argument positions and caller-local claim identities.

An immutable scalar gains a home when borrowed by `observe`; a mutable scalar
is changed from 31 to 47 by `replace`. Both are read after the suspending call.
The frontier must retain their existing homes and the mutable parameter's
current home, not the original SSA values or a newly allocated copy.

The source-selected checked provider completes inline. The requirement still
declares possible suspension, so lowering must retain its exact requirement
identity and checked live bindings rather than rejecting every such call. This is
not evidence of parking, concurrency, cancellation or task settlement.
