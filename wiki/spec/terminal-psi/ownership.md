# Structural claims and cleanup

Structural ownership is an exact live set of roots, projected claims, and
remaining subtrees, not a Boolean property of an aggregate. Borrowed access is
governed separately by [loans](loans.md) and [structural access](structural_access.md).

## Claim paths

An entry claim names a whole structural parameter or a canonical typed path.
Record segments use the exact field identity: an authored numeric ID where
present, otherwise the unnumbered field's spelling. Fixed-array segments use
canonical zero-based literal indexes within a nonempty literal-length shape.
Resolve every segment against the type reached by the previous segment.

The admitted projected-claim vocabulary traverses relevant structural fields
and fixed indexes. Cases, dynamic indexes, scalar/erased leaves, unknown segments,
duplicate paths, ancestor/descendant overlap, and noncanonical order reject.
A projected claim is linear even in an affine aggregate. Several disjoint
linear sibling claims may coexist in one affine root.

Direct calls agree on the complete ordered claim-path set for each structural
argument. Content bindings name the same root/path. Structural returns preserve
a bijection between caller transfers, callee entry claims, returned transfers,
and caller result bindings. Result custody is established only on successful
completion; it cannot also remain on the input owner.

Boundary completion carries the verifier-derived receipt set for every live
claim at each exact argument position, not one receipt per parameter. Missing,
duplicate, reordered, or path-mismatched rows reject. Commit consumption only
after validating successful completion and its result. Rejection leaves the
interpreter's ownership records unchanged; it cannot undo external effects a
host already performed. Suspension must not repeat a committed transfer.

## Partial ownership and residuals

Moving a projection makes its ancestor unavailable for whole-value calls,
returns, or disposal. Duplicate and prefix-overlapping moves reject regardless
of the root's multiplicity. Moving every member of a dense linear sibling claim
set settles that set; it does not license rebuilding an arbitrary value with a
hole.

For an admitted finite record/fixed-array affine root, derive its residual
complement from the type and actual moves. Each row binds root identity, canonical
path, and exact subtree type. List maximal live subtrees in recursive reverse
declaration/index order. An untouched array is one residual subtree, not an
expanded list of leaves. A partially moved ancestor cannot be discarded whole.
An empty complement needs no residual row.

Structural fields carry their required cleanup; primitive and bounded-owned-byte
fields keep their no-cleanup classification. Numeric range or arithmetic policy
does not turn a primitive into an owned structural carrier, and a reference to
that primitive does not inherit owned disposal.

Source establishes fixed-array elements in increasing index order. Ordinary
disposal names the remaining established elements in decreasing order, excluding
moved elements and applying the rule recursively. Partial construction uses
only its established prefix. Producer-supplied order, runtime liveness flags,
or data-dependent cleanup loops cannot replace this static schedule.

Verification reconstructs the complement, rejecting missing/extra rows and
path/type/order drift. Check the required output size before enumerating array
children: a forged huge dimension with a short residual list must not force an
unbounded scan. No-code disposal emits no target instruction or runtime bitmap.

## Roots, continuations, and order

Parameter, named-local, and call-result roots retain distinct establishment
identities. A result ordinal is not a parameter index. An anonymous projected
argument retains its real producer expression rather than a fabricated local;
the producer completes before the consumer even if captured call ordinals are
preorder.

A dying temporary's normal continuation owns its remaining cleanup. A final
consumer may use the return edge; a continuing body uses an explicit continuation
edge. Even an empty complement retains source continuation correspondence.
Partial moves do not retire the whole root early. Other live roots remain live.

Each jump or selected conditional successor has its own exact cleanup/transfer
partition. The transaction validates the current frontier, charges the edge,
materializes scalar successor arguments, disposes the named remainder, then
binds successor parameters. Fuel exhaustion leaves disposal uncommitted.
Scalar arity/types/definitions and ownership are independently checked.
Reconverging paths must establish the same ordered structural frontier; their
scalar values need not be identical.

Audit origin may remain path-sensitive. Operational metadata joins only when
all alternatives use the same realization or an existing runtime discriminator
preserves the distinction, such as a sum tag, provider key, or explicit state.
Otherwise the author must represent the alternative or normalize custody.
The checker cannot duplicate a semantic state to repair an invalid join.

Every incoming owned obligation occurs exactly once in the edge's transfer map,
explicit terminal consumption, eligible automatic cleanup, or validated no-code
affine discard. A dying linear obligation requires an owner-authorized terminal
disposition. The target frontier checks shape, facts, multiplicity, access, and
operational metadata; predecessor origins need not be identical. The complete
partition and conservation evidence remain in proof and diagnostic artifacts.

Calls retain authored order, independently of reverse cleanup order. Whole and
residual cleanup must share a valid establishment-order schedule before a
consumer may support their combination. Transformations unable to preserve an
edge's residuals reject rather than silently dropping them. A crash retains its
abandoned frontier and has no cleanup successor; see [calls and outcomes](calls_and_outcomes.md).

## Nominal cleanup

The attached `T::drop(&mut self)` target is a compiler-owned edge dependency,
not a source-selectable call or static-machine value. Authored early disposal
uses the ordinary consuming `omega::core::drop(value)` call. The cleanup plan
keeps Type-side discharge eligibility, logical prerequisites, operational
reach/effects/work, and derived guarantees separate.

Each action names an exact place, type, and target. Reusing a target for different
places remains multiple actions and multiple work charges. Empty cleanup emits
no call; executable cleanup belongs to its edge/action ordinal and runs before
teardown. The [normal cleanup contract](calls_and_outcomes.md#normal-cleanup)
owns sequential execution and suspension.

Cleanup prerequisites must already be proved locally or authored in `requires`;
analysis cannot add a new caller demand. The bounded proof-only receiver form
substitutes the exact owned place into each target clause and binds each action's
proof independently, not by assuming identical positions in two fact lists.
Its operational target remains zero-argument. Proof-site identities erase at
the verified Psi-to-Omega boundary and cannot reappear downstream.

An owning erased descriptor must retain hidden payload/storage custody, size,
alignment, movement, and the concrete type's eligible cleanup plan. Reconstruct
that relationship under retained facts. Borrowed erased views have no
referent-cleanup disposition. Descriptor lifecycle metadata is not a user-defined
Drop conformance.

Erasure into an automatically cleaned owner requires eligibility under that
owner's retained invariant, or retained stable facts and authority establishing
the concrete plan's prerequisites. Otherwise a linear payload needs an explicit
consuming owner. Erasure neither supplies missing authority nor changes the
disposition. A collection's structural plan invokes each owned erased element's
descriptor plan; borrowed views do not dispose their referents.

## Physical realization

Storage planning consumes the checked transfer map rather than reconstructing
ownership from lexical scopes. Source and target storage may coalesce, but a
borrow promising stable address requires a realization preserving that address.
Cleanup suffix sharing and physical block cloning must retain every action's
exact semantic edge and order. No-code actions still map to the permission ledger.
Coalescing unchanged loop-carried large values is a performance acceptance
requirement, never permission to accept an invalid transfer. An address-stability promise is instead
a soundness obligation, whether realized by coalescing or another valid plan.

Current producer and ABI limits are documented beside
[Terminal production](../../../omega-rust/psi/compiler/terminal-production/README.md#partial-ownership-and-cleanup)
and [native lowering](../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md#structural-results-and-residual-cleanup).
Admitting a Terminal claim or result does not establish native storage or cleanup
support.
