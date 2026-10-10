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

## Retained-loan implementation frontier

`runtime_retained_reference_boundary_exit` pins a retained local reference and
two borrowed leaves in a generic carrier without changing the inline provider.
Source admission closes their actual owned backing and enclosing-invocation
resources before judging carry. The next missing joins concern incoming
referents and complete captured-actual leaf coverage, not a blanket ban on
reference descriptors.

The repair needs one per-value loan-to-backing join, not a permissive reference
type or an exception for an inline provider. Checked `BorrowLoanFact` rows
already retain the source occurrence, captured referent, owner path, available
parent lineage and last use; these are not physical backing or pinning evidence.
The canonical suspension frontier retains the complete current structural
operand, including owned/shared/mutable/write-only access, in its wire bytes
and frontier commitment. That access describes the existing binding; it is
not proof of loan backing or pinning. The checked suspension frontier now
retains exact call-entry loan occurrences, their parent/supplier dependency
closure, logical backing coordinates and captured operand facts. A live loan
retains its backing's independent carry demand, including backing reached
through another carrier; replaced occurrences do not revive old backing.
Captured actual referents and constructed children also retain backing needed
only by the incomplete callee, without a later caller use or named descriptor.
Both sources enter the same backing-liveness closure; the capture tree is not
re-evaluated at invocation.
Incoming referents stay distinct from the parameter slots holding descriptors.
The canonical Terminal graph keeps formation captures, backing, access,
parentage and supplier transport. These are demands, not a preservation proof.
Activation-wide analysis in stage 04 keeps borrow formation's strict CPU/thread
contribution. Runtime backing, preservation and cancellation joins remain
separate requirements.

`fail/concurrency/suspend_reference_permission_without_backing` also refuses
descriptor carry permissions that would otherwise bypass this missing join,
including a generic carrier, a slice and an argument used only by the call.
`fail/concurrency/suspend_with_indirect_resource_backing` pins the independent
resource demand, current replacement, nested backing and call-only constructed
actuals. Portable fields do not erase an explicitly stricter data carry floor.
