# Opaque representation selection

`Build::select_representation<Opaque, Conformance>()` selects one exact named
satisfaction of compiler-owned `OpaqueRepresentation<Opaque>`. A declaration
is inert until selected. The compiler derives its descriptor from the concrete
carrier; Build cannot author sizes, alignment, ABI classes, offsets, movement
rules, or numeric representation IDs. Compiler-owned families such as `Ptr<T>`
derive from pinned TargetSemantics without an authored selection.

## Demand and uniqueness

Reference-only pointees and proof-erased values may remain `Unbound`. An actual
runtime by-value use must close its demand before calling-policy evaluation.
All producers and consumers in that compilation use the same exact application.
Missing, conflicting, lookalike-trait, foreign-target, and shape-invalid choices
reject with demand/selection provenance.

At most one application per opaque declaration may be selected in an activation.
An unused selection occupies that choice but creates no by-value demand row.
An invalid explicit selection still rejects at its source occurrence even if
unused; only absence is demand-driven.
The consumer's authoritative build selects for the combined compilation; it does
not inherit the dependency's historical root-build choice or rerun that build.
Historical review choices do not conflict merely by sharing a source closure.

## Lifecycle and movement

V1 has explicit lifecycle role `Inert`, not an omitted field or provider assertion.
Admission proves no independently invoked cleanup/disposable obligation in any
field, array element type, or sum payload in the complete closed carrier graph,
including inactive cases. A copyable opaque additionally
requires a structurally copyable inert carrier. Affine/linear values retain one
semantic occurrence while lowering copies physical bytes for placement.
Only a checked semantic copy of a copyable opaque creates another occurrence;
a physical copy instruction cannot do so independently.

Cleanup-owning carriers require a separate versioned lifecycle relationship with
total disposition rules; an empty representation trait cannot implicitly inherit
ordinary drop behavior.

## Evidence and composition

| Fact | Meaning |
| --- | --- |
| Producer availability | Exact opaque declaration and public named-conformance/carrier surface; no consumer acceptance. |
| Consumer demand | An actual by-value requirement application under one target and selected carrier. |

Consumer evidence retains opaque declaration, conformance or TargetSemantics
source, concrete carrier, derived shape, physical movement, lifecycle role,
evidence origin, closed-conformance commitment, and complete boundary-plan
commitment. Foreign demands rejoin exact producer canonical rows and immutable
source instances. Compact fingerprints are report coordinates, not identity.

Selecting machine and source occurrence are audit/source provenance, not
application identity. Moving an unchanged selection cannot change ABI agreement.
Independently compiled artifacts compare strong application commitments at actual
by-value composition edges; unrelated artifacts and historical reviews are not
globally unified.

The [review implementation](../../../omega-rust/omega/packages/review/evidence/README.md#record)
retains exact carrier occurrences and checked placement. Runtime representation
agreement is distinct from [package acceptance](../packages/acceptance.md).
