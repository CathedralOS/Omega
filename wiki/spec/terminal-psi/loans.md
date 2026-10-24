# Loan resources and compatibility

Borrow admission joins resource authority with relational evidence. Neither
substitutes for the other.

| Judgment input | Required content |
| --- | --- |
| Loan resource | Exact occurrence, owner lineage, captured place, access, parent lifetime, and restoration obligation. |
| Compatibility | Formation event, captured loan/place identities, access kinds, and the ordering premises available at that use. |

A live exclusive loan does not prove projections disjoint. A proof of
non-interference cannot create a loan, repair missing resource custody, extend
its lifetime, or widen access. Circular justification between these judgments
rejects before either can authorize execution.

Source checking includes
[domain access policy](../language/domains.md#domain-owned-read-and-write-authority)
in the permission judgment: exact subject and normalized domain, requested
read/write axis, authorized machine or requirement invocation, and any delegated
loan's origin and scope. This is not a predicate fact that can be forgotten at a
join. Projection, calls, casts, state transport, and restoration preserve those
restrictions within their governing lifetimes; a bare reference type cannot
manufacture an unrestricted grant.
Helpers receive only checked delegated access. Nothing here requires replaying
successful checking between trusted stages or adding runtime permission storage.

The split is a criterion, not a closed list of obligations. Disjointness,
containment and non-interference are relations over existing, versioned subjects
and may be proved. Loan descent, access attenuation, temporal containment and
restoration are resource judgments. Logical facts cannot create, amplify,
transfer, extend, return, consume or duplicate authority. Proof participates in
ordinary admission from the start; automatic checking derives the same
compatibility judgment rather than preceding a separate proof fallback. Failure
remains an ordinary borrow diagnostic unless source explicitly uses proof
vocabulary. No public `footprint(...)` surface is implied; abstract footprints
for opaque modular APIs remain a separate future facility.

## Captured places and available premises

Loan formation captures a place, not a live selector expression. Retain exact
dynamic-selector coordinates and their checked interpretation at formation.
Compatibility checks those captured places against the loan facts, including
access and the exact formation event. The compiler establishes path availability,
value/place versions and validity scopes when it uses a premise. A premise may
expire after formation without retargeting an already-captured loan.

These are source-language obligations, not a requirement to serialize successful
comparisons and replay them inside the same compile. Under
[verification placement](../../../omega-rust/pipeline.md#verification-placement),
the borrow checker judges the producer's facts once. Proof-carrying artifact
verification is removed; authored mathematics and refinement reasoning remain.

Logical place footprints, semantic `Content<Owner, A>` projections, and physical effect
footprints are different subjects. Relating them requires an explicit checked
bridge, not structural resemblance.

## Reborrow lineage and access

A direct reborrow names its immediate parent resource, exact formation event,
and projection. A multihop chain cannot skip intermediate parents. Availability
immediately before formation is distinct from continued parent activity and
from completed restoration.

The access relation below uses `Read` for shared access:

| Parent | Shared child | Mutable child | Write-only child |
| --- | --- | --- | --- |
| Read | Release child; no exclusive authority restored. | Reject. | Reject. |
| Mutable | Freeze mutation until the complete shared cohort ends; restore once. | Suspend that branch; restore original access. | Suspend that branch; restore original access. |
| Write-only | Reject. | Reject. | Suspend that branch; restore original access. |
| Owned | Freeze mutation in the loaned subtree until the complete shared cohort ends; restore owned access once. | Suspend that subtree; restore owned access. | Suspend that subtree; restore owned access. |

Compatibility, exclusivity, and lifetime premises still apply to each allowed
cell. Attenuation never changes the parent's original access. Close exclusive
lineages deepest-first and release shared cohorts as a set; an omitted cohort
member cannot authorize early restoration.

Lifecycle ordering follows semantic boundaries, not arena indices. Statement
expiry precedes entry; local reassignment ends the old carrier after evaluating
its right-hand side; state exit is last. A parent's lexical retirement before,
with, or after a child is not by itself evidence that authority returned.

## Restoration and publication

Policy introduced solely on a [temporary qualified view](../language/domains.md#temporary-qualified-views)
retires with its complete loan cohort and descendants. Restore the lender's
prior access only after that closure, preserving preexisting owner/field policies
and every independently surviving restriction. A cast or function return does
not establish closure while a derived view remains live. Policy retirement does
not waive custody or invariant-window restoration obligations.

Suspension/freeze containment binds exact child and parent resources, accesses,
activation, parent-entry formation, weakening boundaries, captured places, and
projection remainder. Shared-to-shared release needs no exclusive containment
row. Resource construction preserves the complete lineage and cohort before
committing resource changes; logical ordering facts cannot supply missing members.

Same-boundary lineage closure and state-exit direct-root handoff are distinct
outcomes. The former proves that the final carrier ends at the child's exact
boundary; the latter requires state exit and an exact direct-root lifetime.
Neither label alone establishes restored use, cleanup, transfer, or linear
discharge.

State-exit closure remains a checked resource disposition, not a serialized
Terminal root-handoff certificate. It has no executable consumer. Source
checking still closes exclusive lineages and shared cohorts; lowering must not
restrict accepted loans to a second, exclusive-only publication vocabulary.

A retained dynamic place segment names an opaque selector formation occurrence,
scoped to the handoff's state invocation. Parent segments retain their original
capture identities when a child projects further; only the new suffix belongs
to the child's formation. These identities preserve lineage correspondence,
not scalar equality, disjointness, bounds or wildcard storage authority.
Executable indexed projections retain their ordinary evaluated selectors and
bounds obligations independently of this lifecycle publication.

Lowering consumes the checked lifetime and restoration relationships. The
implementation still carries containment and restored-use records joining a
later operation to its exact source call, parent lifetime and child interval.
Their retirement must migrate those consumers without weakening source borrow
checks; see **BORROW-CERTIFICATES-DELETED** in [the execution board](../../../TASKS.md).

General proof-derived compatibility and broader restoration remain implementation
work. Current supported forms live beside
[borrow checking](../../../omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/README.md).
The [structural access contract](structural_access.md) governs permitted operations
and original-referent preservation independently of these relational proofs.
