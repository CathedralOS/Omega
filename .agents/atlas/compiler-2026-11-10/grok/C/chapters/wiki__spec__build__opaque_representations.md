# Opaque representation selection

Source: `wiki/spec/build/opaque_representations.md` lines 1–72 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
OPAQUE_REPRESENTATIONS1 | "`Build::select_representation<Opaque, Conformance>(builder)` selects one exact named implementation of compiler-owned `OpaqueRepresentation<Opaque, Representation>`, such as `TokenRepresentation satisfies OpaqueRepresentation<Token, TokenBytes> { }`." | Opaque representation selection | Selection must bind exactly one named `OpaqueRepresentation` implementation to the given opaque and conformance pair | not stated | 9
OPAQUE_REPRESENTATIONS2 | "Both data types are ordinary trait arguments; the implementation has no data subject." | Opaque representation selection | The opaque and representation types must be ordinary trait arguments and the implementation must have no data subject | not stated | 7
OPAQUE_REPRESENTATIONS3 | "A declaration is inert until selected." | Opaque representation selection | An unselected opaque representation declaration must not take effect | not stated | 9
OPAQUE_REPRESENTATIONS4 | "The compiler derives its descriptor from the concrete `Representation` carrier; Build cannot author sizes, alignment, ABI classes, offsets, movement rules, or numeric representation IDs." | Opaque representation selection | Descriptor facts must come from the compiler-derived carrier; Build-authored sizes, alignment, ABI classes, offsets, movement rules, or representation IDs must be rejected | not stated | 9
OPAQUE_REPRESENTATIONS5 | "Compiler-owned families such as `Ptr<T>` derive from pinned TargetSemantics without an authored selection." | Opaque representation selection | Compiler-owned families must use pinned TargetSemantics derivation and must not require an authored `select_representation` | not stated | 9
OPAQUE_REPRESENTATIONS6 | "The namespace supplies no receiver." | Opaque representation selection | `select_representation` must not use a receiver on the namespace | not stated | 9
OPAQUE_REPRESENTATIONS7 | "`builder` is an explicit mutable borrow of the current Build activation; an ordinary same-named machine does not select a representation." | Opaque representation selection | Selection must use the explicit `builder` mutable borrow; a same-named machine must not perform selection | not stated | 10
OPAQUE_REPRESENTATIONS8 | "Receiver-style selection is retired." | Opaque representation selection | Receiver-style selection must not be accepted | not stated | 9
OPAQUE_REPRESENTATIONS9 | "Reference-only pointees and proof-erased values may remain `Unbound`." | Demand and uniqueness | Reference-only pointees and proof-erased values may stay without a bound representation application | not stated | 2
OPAQUE_REPRESENTATIONS10 | "An actual runtime by-value use must close its demand before calling-policy evaluation." | Demand and uniqueness | Before calling-policy evaluation, every actual by-value runtime use must have closed representation demand | before calling-policy evaluation | 9
OPAQUE_REPRESENTATIONS11 | "All producers and consumers in that compilation use the same exact application." | Demand and uniqueness | Every producer and consumer in the compilation must use the same exact representation application | not stated | 9
OPAQUE_REPRESENTATIONS12 | "Missing, conflicting, lookalike-trait, foreign-target, and shape-invalid choices reject with demand/selection provenance." | Demand and uniqueness | Missing, conflicting, lookalike-trait, foreign-target, or shape-invalid selections must reject with demand or selection provenance | not stated | 9
OPAQUE_REPRESENTATIONS13 | "At most one application per opaque declaration may be selected in an activation." | Demand and uniqueness | No more than one application per opaque declaration may be selected within one Build activation | not stated | 9
OPAQUE_REPRESENTATIONS14 | "An unused selection occupies that choice but creates no by-value demand row." | Demand and uniqueness | A selected but unused application must consume the one-choice slot and must not create a by-value demand row | not stated | 9
OPAQUE_REPRESENTATIONS15 | "An invalid explicit selection still rejects at its source occurrence even if unused; only absence is demand-driven." | Demand and uniqueness | Invalid explicit selections must reject at the source occurrence even when unused; demand closure must drive only absence cases | not stated | 9
OPAQUE_REPRESENTATIONS16 | "The consumer's authoritative build selects for the combined compilation; it does not inherit the dependency's historical root-build choice or rerun that build." | Demand and uniqueness | Combined compilation must use the consumer build's selection only, not inherited dependency root-build choice or a rerun of that build | not stated | 9
OPAQUE_REPRESENTATIONS17 | "Historical review choices do not conflict merely by sharing a source closure." | Demand and uniqueness | Shared source closure alone must not be treated as a selection conflict between historical review choices | not stated | 9
OPAQUE_REPRESENTATIONS18 | "V1 has explicit lifecycle role `Inert`, not an omitted field or provider assertion." | Lifecycle and movement | Lifecycle V1 must record explicit role `Inert`, not omission or a provider assertion | not stated | 9
OPAQUE_REPRESENTATIONS19 | "Admission proves no independently invoked cleanup/disposable obligation in any field, array element type, or sum payload in the complete closed carrier graph, including inactive cases." | Lifecycle and movement | Admission must prove no independently invoked cleanup or disposable obligation anywhere in the closed carrier graph, including inactive sum cases | at admission | 2
OPAQUE_REPRESENTATIONS20 | "A copyable opaque additionally requires a structurally copyable inert carrier." | Lifecycle and movement | Copyable opaque admission requires a structurally copyable inert carrier | not stated | 2
OPAQUE_REPRESENTATIONS21 | "Affine/linear values retain one semantic occurrence while lowering copies physical bytes for placement." | Lifecycle and movement | Affine or linear opaques must keep one semantic occurrence while lowering may copy physical bytes only for placement | not stated | 2
OPAQUE_REPRESENTATIONS22 | "Only a checked semantic copy of a copyable opaque creates another occurrence; a physical copy instruction cannot do so independently." | Lifecycle and movement | A second semantic occurrence may be created only by a checked semantic copy of a copyable opaque, not by a standalone physical copy | at the operation | 2
OPAQUE_REPRESENTATIONS23 | "Cleanup-owning carriers require a separate versioned lifecycle relationship with total disposition rules; an empty representation trait cannot implicitly inherit ordinary drop behavior." | Lifecycle and movement | Cleanup-owning carriers must have an explicit versioned lifecycle with total disposition; empty representation traits must not inherit ordinary drop implicitly | not stated | 2
OPAQUE_REPRESENTATIONS24 | "Exact opaque declaration and public named-conformance/carrier surface; no consumer acceptance." | Evidence and composition | Producer availability requires the exact opaque declaration and public named-conformance or carrier surface without consumer acceptance | not stated | 9
OPAQUE_REPRESENTATIONS25 | "An actual by-value requirement application under one target and selected carrier." | Evidence and composition | Consumer demand requires an actual by-value requirement application under one target and one selected carrier | not stated | 9
OPAQUE_REPRESENTATIONS26 | "Consumer evidence retains opaque declaration, conformance or TargetSemantics source, concrete carrier, derived shape, physical movement, lifecycle role, evidence origin, closed-conformance commitment, and complete boundary-plan commitment." | Evidence and composition | Consumer evidence must retain all listed fields for the demand | not stated | 9
OPAQUE_REPRESENTATIONS27 | "In a combined source compilation, declaration visibility and the exact selected conformance are checked directly against the retained dependency source; review projections are not compiler inputs." | Evidence and composition | Combined compilation must check declaration visibility and exact selected conformance against retained dependency source only; review projections must not be compiler inputs | not stated | 9
OPAQUE_REPRESENTATIONS28 | "Independently compiled artifact exchanges rejoin exact producer rows and immutable source instances." | Evidence and composition | Artifact exchange must rejoin exact producer rows and immutable source instances | not stated | 9
OPAQUE_REPRESENTATIONS29 | "Compact fingerprints are report coordinates, not identity." | Evidence and composition | Compact fingerprints must be used only as report coordinates, not as application identity | not stated | 9
OPAQUE_REPRESENTATIONS30 | "Selecting machine and source occurrence are audit/source provenance, not application identity." | Evidence and composition | Selecting machine and source occurrence must be recorded as provenance only and must not define application identity | not stated | 9
OPAQUE_REPRESENTATIONS31 | "Moving an unchanged selection cannot change ABI agreement." | Evidence and composition | Relocating an unchanged selection must not alter ABI agreement | not stated | 9
OPAQUE_REPRESENTATIONS32 | "Independently compiled artifacts compare strong application commitments at actual by-value composition edges; unrelated artifacts and historical reviews are not globally unified." | Evidence and composition | Cross-artifact checks must compare strong application commitments only at actual by-value composition edges; must not globally unify unrelated artifacts or historical reviews | at actual by-value composition edges | 9
OPAQUE_REPRESENTATIONS33 | "The [review implementation](../../../omega-rust/omega/packages/review-evidence/README.md#record) retains exact carrier occurrences and checked placement." | Evidence and composition | Review records must retain exact carrier occurrences and checked placement | not stated | 9
OPAQUE_REPRESENTATIONS34 | "Runtime representation agreement is distinct from [package acceptance](../packages/acceptance.md)." | Evidence and composition | Runtime representation agreement checks must be kept separate from package acceptance checks | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"A declaration is inert until selected." | Until selection, the declaration has no runtime or ABI effect and cannot satisfy demand. | "Inert" might mean only lifecycle role `Inert` is not assigned yet, while other compiler bookkeeping still applies. |
"Reference-only pointees and proof-erased values may remain `Unbound`." | The compiler may leave them without a representation application through compilation. | "May" allows but does not require remaining `Unbound`; another rule might still force binding in unstated cases. |
"An unused selection occupies that choice but creates no by-value demand row." | The one-per-declaration slot is consumed even without by-value demand. | "Occupies that choice" might mean only blocking a second selection, not affecting other demand bookkeeping. |
"only absence is demand-driven." | Demand closure is required only when representation is absent, not when an invalid explicit choice is present. | Invalid explicit selections reject at source, but whether other demand paths exist besides absence is not stated. |
"Admission proves no independently invoked cleanup/disposable obligation in any field, array element type, or sum payload in the complete closed carrier graph, including inactive cases." | "Admission" is a compile-time proof obligation before the carrier is admitted. | "Admission" might name a separate review or package step; when the proof must run is not stated in this sentence. |
"Affine/linear values retain one semantic occurrence while lowering copies physical bytes for placement." | Lowering may emit physical copies only when they do not create another semantic occurrence. | Physical copies might still occur for other reasons if not classified as "for placement"; scope of "placement" is unstated. |
"Producer availability | Exact opaque declaration and public named-conformance/carrier surface; no consumer acceptance." (table cell) | Producer side is available when declaration and surface exist, without needing a consumer. | "No consumer acceptance" might forbid any consumer check on the producer side, or only that consumer acceptance is not part of the producer-availability fact. |
"In a combined source compilation, declaration visibility and the exact selected conformance are checked directly against the retained dependency source; review projections are not compiler inputs." | Visibility and conformance must match dependency source text; review-projected views must be ignored. | "Checked directly" might allow equivalence checks beyond literal source retention; what "retained dependency source" includes is unstated. |
"Moving an unchanged selection cannot change ABI agreement." | Changing only provenance fields of a selection must not change ABI. | "Moving" might mean moving values at runtime rather than relocating metadata; ABI invariance scope is unstated. |
"The [review implementation](../../../omega-rust/omega/packages/review-evidence/README.md#record) retains exact carrier occurrences and checked placement." | The compiler or review pipeline must persist those fields in review records. | This sentence describes an external review implementation; whether the compiler must enforce the same retention is not stated in this line. |

## Terms used without definition in this chapter

- `Build::select_representation`
- `Opaque` / `Conformance` (type parameters)
- `builder`
- compiler-owned
- `OpaqueRepresentation<Opaque, Representation>`
- `Representation` (carrier)
- data subject
- descriptor
- sizes / alignment / ABI classes / offsets / movement rules
- numeric representation IDs
- `Ptr<T>`
- pinned TargetSemantics
- authored selection
- namespace (receiver context)
- Build activation
- same-named machine
- receiver-style selection
- reference-only pointees
- proof-erased values
- `Unbound`
- by-value use / by-value demand
- calling-policy evaluation
- producers and consumers
- exact application
- lookalike-trait
- foreign-target
- shape-invalid
- demand/selection provenance
- opaque declaration
- activation (Build activation)
- by-value demand row
- demand-driven
- authoritative build
- combined compilation
- historical root-build choice
- historical review choices
- source closure
- lifecycle role `Inert` / V1
- provider assertion
- admission
- cleanup/disposable obligation
- closed carrier graph
- inactive cases (sum)
- copyable opaque
- structurally copyable inert carrier
- affine/linear values
- semantic occurrence
- physical copy instruction
- checked semantic copy
- cleanup-owning carriers
- versioned lifecycle relationship
- total disposition rules
- empty representation trait
- ordinary drop behavior
- Producer availability / Consumer demand
- public named-conformance/carrier surface
- consumer acceptance
- Consumer evidence
- TargetSemantics source
- derived shape
- physical movement
- evidence origin
- closed-conformance commitment
- boundary-plan commitment
- combined source compilation
- declaration visibility
- review projections
- retained dependency source
- independently compiled artifact exchanges
- producer rows
- immutable source instances
- compact fingerprints
- report coordinates
- application identity
- selecting machine
- source occurrence
- audit/source provenance
- ABI agreement
- strong application commitments
- by-value composition edges
- review implementation
- exact carrier occurrences
- checked placement
- runtime representation agreement
- package acceptance

CHAPTER COMPLETE
