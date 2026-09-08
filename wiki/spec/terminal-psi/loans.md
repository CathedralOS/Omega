# Loan resources and compatibility

Borrow admission joins resource authority with relational evidence. Neither
substitutes for the other.

| Record | Required content |
| --- | --- |
| Loan resource | Exact occurrence, owner lineage, captured place, access, parent lifetime, and restoration obligation. |
| Compatibility certificate | Formation event, captured loan/place identities, normalized relational conclusion, exact premise tokens, and checked derivation. |

A live exclusive loan does not prove projections disjoint. A proof of
non-interference cannot create a loan, repair missing resource custody, extend
its lifetime, or widen access. Circular justification between these records
rejects before either can authorize execution.

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

## Frozen places and proof replay

Loan formation captures a place, not a live selector expression. Retain exact
dynamic-selector coordinates and their checked interpretation at formation.
Replaying compatibility checks those frozen places against the resource rows,
including authoritative access and the exact formation event.

The verifier reconstructs dominance, path availability, value/place versions,
and validity scopes from premise establishment points. It does not trust a
serialized assertion that a premise dominates or is valid. Replay the derivation
and its exact conclusion. A premise may expire after formation without
retargeting an already-captured loan or invalidating the established relation
between those frozen occurrences.

Logical place footprints, semantic `Content<A>` projections, and physical effect
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

Compatibility, exclusivity, and lifetime premises still apply to each allowed
cell. Attenuation never changes the parent's original access. Close exclusive
lineages deepest-first and release shared cohorts as a set; an omitted cohort
member cannot authorize early restoration.

Lifecycle ordering follows semantic boundaries, not arena indices. Statement
expiry precedes entry; local reassignment ends the old carrier after evaluating
its right-hand side; state exit is last. A parent's lexical retirement before,
with, or after a child is not by itself evidence that authority returned.

## Restoration and publication

Suspension/freeze containment binds exact child and parent resources, accesses,
activation, parent-entry formation, weakening boundaries, captured places, and
projection remainder. Shared-to-shared release needs no exclusive containment
row. Replayed rosters reject omissions, duplicates, reordering, amplification,
and retargeting before committing resource changes.

Same-boundary lineage closure and state-exit direct-root handoff are distinct
outcomes. The former proves that the final carrier ends at the child's exact
boundary; the latter requires state exit and an exact direct-root lifetime.
Neither label alone establishes restored use, cleanup, transfer, or linear
discharge.

Terminal publication independently replays lineage, access, semantic boundaries,
projection, and containment. A restored-use row additionally binds the exact
later operation, source call coordinate, callee, root lifetime, restoration
class, child interval, and complete shared cohort where applicable. Its authority
is limited to that named use. Committed source-call identity is not reconstructed
from machine bytes. Checked arenas alone are not portable authority.

General proof-derived compatibility and broader restoration publication remain
implementation work. Current supported forms and source correspondence live
beside [borrow checking](../../../omega-rust/psi/pipeline/typed-trees-to-checked-trees/README.md).
The [structural access contract](structural_access.md) governs permitted operations
and original-referent preservation independently of these relational proofs.
