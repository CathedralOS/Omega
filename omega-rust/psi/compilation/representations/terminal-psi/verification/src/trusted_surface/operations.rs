//! Operation-schema entries: one per `OperationSemanticTag`.
//!
//! Each entry records how reconstruction treats the operation: goal-free leaf
//! denotation, proof-bearing canonical obligation, structural-effect
//! observation, call composition, or no proposition facts. The exhaustive map
//! has no wildcard arm: a new operation tag fails compilation here before
//! publication.

use terminal_semantics::OperationSemanticTag;

use super::{CoveredSurface, EntryBinding, LedgerFamily, SoundnessStatus, TrustedSurfaceEntry};

const VOCAB: &str = "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/operations.rs";
const FLOAT_POLICY: &str = "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/float_result_policy.rs";
const TS_ROWS: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs";
const TS_LEAF_SCHEMA: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_leaf_schema.rs";
const TS_LEAF_SEMANTICS: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_leaf_semantics.rs";
const TS_PBS: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar.rs";
const TS_PBS_GOAL: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar/canonical_goal.rs";
const TS_SE: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect.rs";
const TS_SE_BYTE_PREDICATE: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_predicate.rs";
const TS_SE_BYTE_EQUAL: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_equal.rs";
const TS_SE_EXTENT: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_extent.rs";
const TS_CALLS: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition.rs";
const TS_CALLS_VIEW: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/fixed_byte_view.rs";
const TS_RECORD: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/record_field.rs";
const TS_SCALAR_ARRAY: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_array.rs";
const TS_ZEROED_STRUCTURAL: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/zeroed_structural.rs";
const OP_FACTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts.rs";
const OP_FACTS_PRIMITIVE_ELEMENTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/primitive_elements.rs";
const OP_FACTS_FLOAT_FIELDS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/float_fields.rs";
const OP_FACTS_CASE_HOPS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/case_hops.rs";
const OP_FACTS_POLARITY: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/boolean_polarity.rs";
const OP_FACTS_BYTE_EXTENT: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/byte_extent.rs";
const OP_FACTS_ELEMENT_EXTENT: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/element_extent.rs";
const TS_SE_ELEMENT_EXTENT: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/element_extent.rs";
const TS_ELEMENT_REPRESENTATION: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/element_representation.rs";
const TS_REPRESENTATION_REGIONS: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/representation_regions.rs";
const TS_COLLECTION_BACKING: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/collection_backing.rs";
const VAL_ELEMENTS_ESTABLISH: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/establishment.rs";
const VAL_ELEMENTS_LENGTH: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/length.rs";
const VAL_ELEMENTS_READ: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/read.rs";
const VAL_ELEMENTS_WRITE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/write.rs";
const VAL_ELEMENTS_MUTABLE_ESTABLISH: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/mutable_establishment.rs";
const VAL_ELEMENTS_SUBSLICE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/subslice.rs";
const OP_FACTS_RECORD: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/record.rs";
const OP_FACTS_SCALAR_CASE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/scalar_case.rs";
const OP_FACTS_STRUCTURAL_CASE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/structural_case.rs";
const TV_CALLS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/call_composition.rs";
const TS_SHARED_MUTATION: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/shared_mutation.rs";
const TV_STORAGE_OBSERVATIONS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/storage_observations.rs";
const VAL_OPS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/operations.rs";
const VAL_RECORD: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/record.rs";
const VAL_SCALAR_CASE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/case.rs";
const VAL_STRUCTURAL_CASE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case.rs";
const VAL_SCALAR_ARRAY: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/array.rs";
const VAL_CASE_MEMBERSHIP: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case_membership.rs";
const VAL_LEAF_COPY: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/leaf_copy.rs";
const VAL_PRIMITIVE_STORAGE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/primitive_storage.rs";
const TS_PRIMITIVE_PLACE: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/primitive_place.rs";
const VAL_REFERENCES: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/references.rs";
const VAL_STRUCTURAL_OPS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_paths.rs";
const VAL_UNIT_OPERATION: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/unit_operation.rs";
const VAL_BORROWED_WINDOWS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/borrowed_windows.rs";
const VAL_STRUCTURAL_SCALAR: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/scalar_fields.rs";
const VAL_STRUCTURAL_BYTES: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields.rs";
const VAL_STRUCTURAL_BYTES_STORE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_store.rs";
const VAL_STRUCTURAL_BYTE_VIEW_STORE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_view_field_store.rs";
const VAL_BYTES_LENGTH: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/length.rs";
const VAL_BYTES_READ: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/read.rs";
const VAL_BYTES_WRITE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/write.rs";
const VAL_BYTES_SUBSLICE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/subslice.rs";
const VAL_BYTES_PREDICATE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/predicate.rs";
const VAL_BYTES_EQUAL: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/equal.rs";
const VAL_FLOAT: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/float_meaning.rs";
const VAL_PARTIAL_AFFINE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/partial_affine.rs";
const VAL_CONTRACTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/contracts.rs";
const VAL_DYNAMIC: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/dynamic_dispatch.rs";
const VAL_TRAPPING: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/trapping_integer.rs";
const VAL_FLOAT_POLICY: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/float_result_policy.rs";
const VAL_ATOMIC: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/atomic_access.rs";
const TS_REFERENCE_VIEWS: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/reference_views.rs";
const TS_REPRESENTATION_VIEW: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_representation_view.rs";
const VAL_REPRESENTATION_VIEW: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar_representation_view.rs";
const VAL_SUSPENSION: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/suspension_call_plan.rs";

const TRUSTED: SoundnessStatus = SoundnessStatus::ExplicitlyTrusted {
    root: "root:rust-reference-verifier",
    rationale: "schema row and reconstruction arm witnessed by the verifier's operation corpus and rejection tests; no lower-rung derivation discharges them",
};

const fn entry(
    id: &'static str,
    premises: &'static str,
    conclusion: &'static str,
    dependencies: &'static [&'static str],
    implementation: &'static [&'static str],
) -> TrustedSurfaceEntry {
    TrustedSurfaceEntry {
        id,
        family: LedgerFamily::OperationSchema,
        binding: EntryBinding::DispatchOn(CoveredSurface::OperationTags),
        premises,
        conclusion,
        dependencies,
        implementation,
        soundness: TRUSTED,
    }
}

const GOAL_FREE_DEPS: &[&str] = &[
    "fact:goal-free-scalar-result",
    "fact:scalar-carrier-bounds",
    "formation:operation-validation",
];
const GOAL_FREE_BOOLEAN_DEPS: &[&str] = &[
    "fact:goal-free-scalar-result",
    "fact:boolean-polarity-implications",
    "fact:scalar-carrier-bounds",
    "formation:operation-validation",
];
const PROOF_BEARING_DEPS: &[&str] = &[
    "fact:proof-bearing-scalar-goal",
    "owner:operation",
    "formation:operation-validation",
];
const EFFECT_DEPS: &[&str] = &[
    "fact:structural-effect-observation",
    "formation:operation-validation",
];
const CALL_DEPS: &[&str] = &[
    "composition:call-instantiation",
    "formation:operation-validation",
    "formation:active-case-projection",
];
const NO_FACT_DEPS: &[&str] = &["formation:operation-validation"];

const GOAL_FREE_SITES: &[&str] = &[
    VOCAB,
    TS_ROWS,
    TS_LEAF_SCHEMA,
    TS_LEAF_SEMANTICS,
    OP_FACTS,
    VAL_OPS,
];
const GOAL_FREE_BOOLEAN_SITES: &[&str] = &[
    VOCAB,
    TS_ROWS,
    TS_LEAF_SCHEMA,
    TS_LEAF_SEMANTICS,
    OP_FACTS,
    OP_FACTS_POLARITY,
    VAL_OPS,
];
const PROOF_BEARING_SITES: &[&str] = &[VOCAB, TS_ROWS, TS_PBS, TS_PBS_GOAL, OP_FACTS, VAL_OPS];
const CALL_SITES: &[&str] = &[
    VOCAB,
    TS_ROWS,
    TS_CALLS,
    TS_CALLS_VIEW,
    TV_CALLS,
    TS_SHARED_MUTATION,
    TV_STORAGE_OBSERVATIONS,
    OP_FACTS,
    OP_FACTS_CASE_HOPS,
    VAL_STRUCTURAL_OPS,
    VAL_OPS,
    VAL_CONTRACTS,
];
const NO_FACT_SITES: &[&str] = &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS];

// -- Structural-effect observation rows --

static OP_ESTABLISH_REFERENCE: TrustedSurfaceEntry = entry(
    "operation:establish-reference",
    "a validated source place whose authority covers the declared reference access; exact referent identity or an independently reconstructed total shared primitive representation view preserves the original backing, never a copied scalar; overlapping formations must all be shared, and each carrier retains its own identity until the entire cohort ends before a source write; every Case/Field ingress projection retains its original root and proves each case active before formation",
    "the reference-establishment observation publishes no scalar equation; Case premises are checked before invalidation, and mutable record formation retires source-root observations because later writes can use a different carrier identity; release restores permission, not prior contents",
    &[
        "fact:structural-effect-observation",
        "formation:operation-validation",
        "formation:active-case-projection",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_REFERENCES,
        TS_REFERENCE_VIEWS,
        TS_REPRESENTATION_VIEW,
        OP_FACTS_CASE_HOPS,
        VAL_STRUCTURAL_OPS,
    ],
);
static OP_RELEASE_REFERENCE: TrustedSurfaceEntry = entry(
    "operation:release-reference",
    "a validated established reference released by this operation",
    "the reference-release observation ending the reference's validity",
    EFFECT_DEPS,
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_REFERENCES],
);
static OP_ESTABLISH_PRIMITIVE_LOCAL: TrustedSurfaceEntry = entry(
    "operation:establish-primitive-local",
    "a validated primitive local initialization, structural or scalar",
    "the establishment observation; a structural result also invalidates every proposition observing the result place",
    &[
        "fact:structural-effect-observation",
        "invalidation:establish-primitive-local",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_PRIMITIVE_STORAGE,
    ],
);
static OP_PRIMITIVE_SCALAR_READ: TrustedSurfaceEntry = entry(
    "operation:primitive-scalar-read",
    "a readable live root and a relevant-field/fixed-array path to initialized primitive storage whose elements may be runtime-selected",
    "the read observation's local equation where the schema declares one and the path is static; an exact-format static IEEE field also yields result-to-field bit identity, preserving NaN payloads and signed zero rather than asserting IEEE equality; a static primitive element yields result-to-element equality at its canonical path; each runtime element's bound is the operation's own obligation",
    &[
        "fact:structural-effect-observation",
        "fact:runtime-index-bound",
        "fact:ieee-primitive-field-observation",
        "fact:primitive-element-observation",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        OP_FACTS_FLOAT_FIELDS,
        OP_FACTS_PRIMITIVE_ELEMENTS,
        VAL_OPS,
        VAL_PRIMITIVE_STORAGE,
        TS_PRIMITIVE_PLACE,
    ],
);
static OP_STRUCTURAL_CASE_MEMBERSHIP: TrustedSurfaceEntry = entry(
    "operation:structural-case-membership",
    "a readable live whole root and validated field/index path to the exact nominal sum case",
    "the exact root/path/case Boolean observation and its conditional membership fact; writes invalidate that place-bearing fact, and observing the discriminator alone grants no payload access",
    EFFECT_DEPS,
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_CASE_MEMBERSHIP,
    ],
);
static OP_STRUCTURAL_LEAF_COPY: TrustedSurfaceEntry = entry(
    "operation:structural-leaf-copy",
    "a readable live whole root and validated field/index path to an Unrestricted leaf, with every element-view selector retaining a runtime-index live-length obligation, or to an existing shared-reference descriptor that the copy relocates; a whole bounded-byte copy has the exact source carrier and primitive qualification roster, with no new establishment evidence or claims",
    "an Unrestricted result is a fresh owned exact copy and the source stays fully intact: prior observations of the result place retire, and when the source path is a canonical static field/index path each scalar leaf of a copied record equals the source's leaf at that path (IEEE leaves as exact-format bit-identity equations, never IEEE equality); ordinary write invalidation retires an equation when either root's leaf is written; a runtime-selected source path publishes no equation; a SharedBorrow result relocates an existing reference descriptor with its original loan origin and publishes no equation; borrowing a new payload uses an ordinary borrowed argument or EstablishReference, never an implicit owned snapshot or a new loan hidden in a copy",
    &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        OP_FACTS,
        OP_FACTS_RECORD,
        VAL_OPS,
        VAL_LEAF_COPY,
    ],
);
static OP_STRUCTURAL_CASE_LEAF_COPY: TrustedSurfaceEntry = entry(
    "operation:structural-case-leaf-copy",
    "a readable live whole root and a canonical path entering at least one case; every case hop has exact root/prefix membership in the reconstructed incoming facts, or every other declared case of that sum is refuted by a surviving membership observation of the same place whose result is known false, before copying an Unrestricted leaf",
    "no proposition facts: a fresh owned place holding an exact copy of the projected case-leaf; the source stays fully intact",
    &[
        "formation:operation-validation",
        "fact:semantic-axiom-roster",
        "operation:structural-case-membership",
        "fact:structural-case-arm",
        "conversion:value-equality-denotation",
        "rule:semantic-axiom",
        "rule:implication-elimination",
    ],
    &[
        VOCAB,
        TS_ROWS,
        OP_FACTS,
        OP_FACTS_CASE_HOPS,
        VAL_OPS,
        VAL_LEAF_COPY,
    ],
);
static OP_WRITE_ONLY_PRIMITIVE_STORE: TrustedSurfaceEntry = entry(
    "operation:write-only-primitive-store",
    "a writable live root, a relevant-field/fixed-array path to a primitive leaf whose elements may be runtime-selected, and exactly typed dominating scalar value",
    "the store observation; after scalar snapshot capture, every remaining proposition observing the destination is invalidated; an exact-format static IEEE field then yields field-to-stored-value bit identity, never IEEE equality or a finiteness claim; a static primitive element then yields element-to-stored-value equality at its canonical path; each runtime element's bound is the operation's own obligation",
    &[
        "fact:structural-effect-observation",
        "fact:runtime-index-bound",
        "invalidation:write-only-primitive-store",
        "fact:ieee-primitive-field-observation",
        "fact:primitive-element-observation",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        OP_FACTS_FLOAT_FIELDS,
        OP_FACTS_PRIMITIVE_ELEMENTS,
        VAL_OPS,
        VAL_PRIMITIVE_STORAGE,
        TS_PRIMITIVE_PLACE,
    ],
);
static OP_STRUCTURAL_SCALAR_FIELD_STORE: TrustedSurfaceEntry = entry(
    "operation:structural-scalar-field-store",
    "a validated scalar field store of a record field or a mixed shape's common field (never a payload field), whose carrier may cross runtime elements each owning a bound obligation, with a range obligation exactly when its destination declaration is a bounded integer",
    "the exact stored SSA value's declared bounds and each runtime element's `index < extent` are required against pre-write axioms; then propositions observing the write are invalidated (the whole root when the carrier crosses a runtime element) and a resolvable leaf publishes its stored-value equation",
    &[
        "fact:structural-effect-observation",
        "fact:runtime-index-bound",
        "invalidation:structural-field-store",
        "fact:field-store-leaf-equation",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_OPS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/scalar_fields.rs",
    ],
);
static OP_MOVE_STRUCTURAL_FIELD: TrustedSurfaceEntry = entry(
    "operation:move-structural-field",
    "a live mutable-borrowed machine parameter, an eligible completed owned home, or a mutable-borrowed block carrier whose possible origins all have that authority; a canonical field path to a declared non-erased structural subtree with no overlapping restoration debt or observation; a root window (no field, empty path) names only a mutable-borrowed machine or block loan, and its result carries the referent's exact type",
    "the extraction observation; the subtree leaves its borrowed home as restoration debt until an exact-type repair store reseats it, and every proposition observing the vacated path is invalidated; a root window's debt is the empty path, so every observation of the root overlaps it, and an affine displaced value keeps its disposal obligation on the frontier",
    &[
        "invalidation:structural-field-store",
        "fact:borrowed-storage-restoration-debt",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_OPS,
        VAL_BORROWED_WINDOWS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_window.rs",
    ],
);
static OP_STORE_STRUCTURAL_FIELD: TrustedSurfaceEntry = entry(
    "operation:store-structural-field",
    "an open restoration-debt hole under the exact carrier that opened it and a consumed same-typed value reseating it; possible aliasing alone does not establish repair identity; a root repair names only the mutable-borrowed loan whose whole referent it vacated",
    "the repair observation closing the exact debt; propositions observing the reseated path are invalidated and no field equation survives the move boundary",
    &[
        "invalidation:structural-field-store",
        "fact:borrowed-storage-restoration-debt",
        "owner:operation",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_OPS,
        VAL_BORROWED_WINDOWS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_window.rs",
    ],
);
static OP_BYTE_SEQUENCE_STORE: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-store",
    "an exclusive bounded-owner parameter, immutable completed source with exact current length, and independently replayed preservation of every declared byte predicate",
    "source length at most destination capacity is reconstructed before mutation; old destination observations are invalidated and caller backing receives the complete snapshot",
    &[
        "fact:structural-effect-observation",
        "invalidation:structural-field-store",
        "fact:byte-extent-length",
        "owner:operation",
        "formation:operation-validation",
        "formation:byte-sequence-operations",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/store.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/call_composition/bounded_byte_owner.rs",
    ],
);
static OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE: TrustedSurfaceEntry = entry(
    "operation:structural-byte-sequence-field-store",
    "a validated byte-sequence field store with declared capacity and length obligation",
    "propositions observing the write are invalidated and the length-at-most-capacity obligation is reconstructed",
    &[
        "fact:structural-effect-observation",
        "invalidation:structural-field-store",
        "fact:byte-extent-length",
        "owner:operation",
        "formation:operation-validation",
        "formation:structural-byte-sequence-fields",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_BYTES_STORE,
    ],
);
static OP_STRUCTURAL_BYTE_VIEW_FIELD_STORE: TrustedSurfaceEntry = entry(
    "operation:structural-byte-view-field-store",
    "a validated descriptor replacement of a shared borrowed-view field through writable storage, with the exact current runtime descriptor or literal source, independent source availability, and body-derived backing origins",
    "propositions observing the replaced field are invalidated; the exact slot acquires the source descriptor and its live backing obligations while siblings and detached copies retain theirs; source predicates imply the field restrictions directly or a separately reconstructed operation obligation proves the exact source pre-store content, never the old field invariant or backing alias equality; literal image residence is one source lifetime, and ordinary call writeback and normal-return escape are independently checked by the descriptor frontier",
    &[
        "fact:structural-effect-observation",
        "invalidation:structural-field-store",
        "owner:operation",
        "formation:operation-validation",
        "formation:frontier",
        "formation:structural-byte-sequence-fields",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_BYTE_VIEW_STORE,
    ],
);
static OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH: TrustedSurfaceEntry = entry(
    "operation:structural-byte-sequence-field-length",
    "a validated byte-sequence field length read",
    "the observation plus the length equation when the field's extent resolves",
    &[
        "fact:structural-effect-observation",
        "fact:byte-extent-length",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_BYTES,
    ],
);
static OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE: TrustedSurfaceEntry = entry(
    "operation:structural-byte-sequence-field-byte-store",
    "a validated single-byte store into a byte-sequence field with a current same-field length; a UTF-8 destination not declared ASCII-only is all-ASCII on every path into the store",
    "propositions observing the write are invalidated; no scalar equation is published; the goal is index < length plus the stored byte's class legs its destination's declared predicates require (value <= 127 for ASCII-only or UTF-8, 1 <= value for no-NUL), so every declared predicate still holds after the store",
    &[
        "fact:structural-effect-observation",
        "invalidation:structural-field-store",
        "formation:operation-validation",
        "formation:structural-byte-sequence-fields",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_BYTES,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
    ],
);
static OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_READ: TrustedSurfaceEntry = entry(
    "operation:structural-byte-sequence-field-read",
    "a validated single-byte read of a byte-sequence field",
    "the read observation's bound obligation is reconstructed against the field's own length",
    &[
        "fact:structural-effect-observation",
        "fact:byte-extent-length",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_BYTES,
    ],
);
static OP_ESTABLISH_SCALAR_CASE: TrustedSurfaceEntry = entry(
    "operation:establish-scalar-case",
    "a validated scalar-case establishment over a sum type",
    "the case-establishment facts and declared obligations",
    &[
        "fact:scalar-case-establishment",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        OP_FACTS_SCALAR_CASE,
        VAL_OPS,
        VAL_SCALAR_CASE,
    ],
);
static OP_ESTABLISH_STRUCTURAL_CASE: TrustedSurfaceEntry = entry(
    "operation:establish-structural-case",
    "a validated structural-case establishment over a sum type, or over a mixed type whose runtime common fields are relevant scalars, binding those common fields in declaration order and then the selected case's runtime payload; erased proof members, common or payload, take no roster position",
    "the case-establishment facts and declared obligations",
    &[
        "fact:structural-case-establishment",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        OP_FACTS_STRUCTURAL_CASE,
        VAL_OPS,
        VAL_STRUCTURAL_CASE,
    ],
);
static OP_ESTABLISH_SCALAR_ARRAY: TrustedSurfaceEntry = entry(
    "operation:establish-scalar-array",
    "a validated scalar array with complete typed initialization",
    "no proposition facts: the total validation judgment carries the authority",
    &["formation:operation-validation"],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_SCALAR_ARRAY,
        TS_SCALAR_ARRAY,
    ],
);
static OP_ESTABLISH_BYTE_SEQUENCE_CONCATENATION: TrustedSurfaceEntry = entry(
    "operation:establish-byte-sequence-concatenation",
    "two readable byte sources with exact dominating live-length observations and an independent capacity obligation; the claim-free unrestricted bounded-owned result's exact primitive byte qualifications must follow from the inputs",
    "fresh independent owned bytes in operand order with the validated declared qualifications; a later length observation equals the exact sum, not capacity; indexed writes preserve the declaration rather than freezing incidental initializer predicates",
    &[
        "fact:byte-extent-length",
        "formation:operation-validation",
        "formation:byte-sequence-operations",
    ],
    &[
        VOCAB,
        TS_ROWS,
        OP_FACTS,
        VAL_OPS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_concatenation.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/concatenation.rs",
    ],
);
static OP_ESTABLISH_BYTE_SEQUENCE_LITERAL: TrustedSurfaceEntry = entry(
    "operation:establish-byte-sequence-literal",
    "a validated byte-sequence literal establishment carrying the byte-predicate domain memberships its check discharged as occurrence qualifications",
    "the literal's extent observation where the schema declares one",
    &[
        "fact:structural-effect-observation",
        "fact:byte-extent-length",
        "formation:operation-validation",
        "formation:structural-operations",
    ],
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS],
);
static OP_BYTE_SEQUENCE_LENGTH: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-length",
    "a validated byte-sequence length read",
    "the length observation's local equation where the schema declares one",
    &[
        "fact:structural-effect-observation",
        "fact:byte-extent-length",
        "formation:operation-validation",
    ],
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_BYTES_LENGTH],
);
static OP_BYTE_SEQUENCE_WRITE: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-write",
    "a byte-sequence write through an exact unqualified mutable view or an established claim-free unrestricted bounded-owned result, with a current same-source length; a UTF-8 owner not declared ASCII-only must have all-ASCII backing on every path from its establishment",
    "the independent index < length goal includes the exact owner's declared byte-class legs: value <= 127 for ASCII-only or UTF-8, and 1 <= value for no-NUL; the write preserves live length and declared predicates, discards incidental initializer predicates, and invalidates propositions observing the destination extent",
    &[
        "fact:structural-effect-observation",
        "invalidation:byte-sequence-write",
        "formation:operation-validation",
        "formation:byte-sequence-operations",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_BYTES_WRITE,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields/freshness.rs",
    ],
);
static OP_BYTE_SEQUENCE_READ: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-read",
    "a validated byte-sequence read with a current same-source length and its independent indexing obligation",
    "the read's immutable u8 snapshot receives only the reconstructed no-NUL or ASCII class bounds its source supports; UTF-8 alone supplies no ASCII fact",
    &[
        "fact:structural-effect-observation",
        "fact:byte-read-class",
        "formation:operation-validation",
    ],
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_BYTES_READ],
);
static OP_BYTE_SEQUENCE_SUBSLICE: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-subslice",
    "a validated byte-sequence subslice over a known extent",
    "the subslice observation and its extent equation where the schema declares one; shared byte membership separately reconstructs only subslice-preserving source predicates without result qualification annotations",
    &[
        "fact:structural-effect-observation",
        "fact:byte-extent-length",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_SE_EXTENT,
        OP_FACTS,
        OP_FACTS_BYTE_EXTENT,
        VAL_OPS,
        VAL_BYTES_SUBSLICE,
    ],
);
static OP_BYTE_SEQUENCE_PREDICATE: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-predicate",
    "a validated byte-sequence predicate evaluation over the operand's exact live extent",
    "the operand's own dominating current length observation is the read bound; the Boolean result equals the content predicate over that exact descriptor root and canonical path, without identifying aliased windows or assuming the predicate is true; the observation owes no index certificate",
    &[
        "fact:structural-effect-observation",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_SE_BYTE_PREDICATE,
        OP_FACTS,
        VAL_OPS,
        VAL_BYTES_PREDICATE,
    ],
);
static OP_BYTE_SEQUENCE_EQUAL: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-equal",
    "a validated content-equality evaluation over two operands' exact live extents",
    "each operand's own dominating current length observation is its read bound; the Boolean result equals live-length equality and byte equality at every live position of the two exact descriptors, without identifying aliased windows; a result-true premise concludes the proposition's field-resolved equality; the observation owes no index certificate",
    &[
        "fact:structural-effect-observation",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_SE_BYTE_EQUAL,
        OP_FACTS,
        VAL_OPS,
        VAL_BYTES_EQUAL,
    ],
);
static OP_ESTABLISH_ELEMENT_VIEW: TrustedSurfaceEntry = entry(
    "operation:establish-element-view",
    "a validated shared element-view establishment over an exact-element collection, a normalized typed collection whose source leaf bit sets imply target validity, or a total normalized representation region of original fixed byte backing",
    "the establishment observation; an exact u8 representation region may produce the ordinary borrowed-byte descriptor; a representation region independently requires ordered u64 start <= end <= backing extent and exact suffix tiling by the reconstructed nonzero element stride; no host alignment or backing ownership follows",
    EFFECT_DEPS,
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_ELEMENTS_ESTABLISH,
        TS_COLLECTION_BACKING,
        TS_ELEMENT_REPRESENTATION,
        TS_REPRESENTATION_REGIONS,
        OP_FACTS_ELEMENT_EXTENT,
    ],
);
static OP_ESTABLISH_MUTABLE_ELEMENT_VIEW: TrustedSurfaceEntry = entry(
    "operation:establish-mutable-element-view",
    "a validated mutable element-view establishment over an exact-element collection, a normalized typed collection with equivalent leaf bit sets and padding geometry and no overlapping source occurrence qualifications, or a total normalized representation region of original fixed byte backing",
    "the establishment observation; an exact u8 representation region may produce the ordinary borrowed-byte descriptor; a representation region independently requires ordered u64 start <= end <= backing extent and exact suffix tiling by the reconstructed nonzero element stride; no host alignment or backing ownership follows",
    EFFECT_DEPS,
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_ELEMENTS_MUTABLE_ESTABLISH,
        VAL_ELEMENTS_ESTABLISH,
        TS_COLLECTION_BACKING,
        TS_ELEMENT_REPRESENTATION,
        TS_REPRESENTATION_REGIONS,
        OP_FACTS_ELEMENT_EXTENT,
    ],
);
static OP_ELEMENT_VIEW_LENGTH: TrustedSurfaceEntry = entry(
    "operation:element-view-length",
    "a validated element-view length observation",
    "the length observation's local equation where the schema declares one",
    &[
        "fact:structural-effect-observation",
        "fact:element-extent-length",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_SE_ELEMENT_EXTENT,
        TS_REPRESENTATION_REGIONS,
        OP_FACTS,
        OP_FACTS_ELEMENT_EXTENT,
        VAL_OPS,
        VAL_ELEMENTS_LENGTH,
    ],
);
static OP_ELEMENT_VIEW_READ: TrustedSurfaceEntry = entry(
    "operation:element-view-read",
    "a validated in-bounds element read at the element's scalar leaf",
    "the read observation's local equation where the schema declares one",
    EFFECT_DEPS,
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_SE_ELEMENT_EXTENT,
        OP_FACTS,
        VAL_OPS,
        VAL_ELEMENTS_READ,
    ],
);
static OP_ELEMENT_VIEW_WRITE: TrustedSurfaceEntry = entry(
    "operation:element-view-write",
    "a validated mutable element-view store at an exact static scalar leaf path, with an index bound and both declared bounded-integer endpoints proved in the single write obligation",
    "the write observation; propositions observing the destination extent are invalidated",
    &[
        "fact:structural-effect-observation",
        "invalidation:element-view-write",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_SE_ELEMENT_EXTENT,
        OP_FACTS,
        OP_FACTS_ELEMENT_EXTENT,
        VAL_OPS,
        VAL_ELEMENTS_WRITE,
    ],
);
static OP_ELEMENT_VIEW_SUBSLICE: TrustedSurfaceEntry = entry(
    "operation:element-view-subslice",
    "a validated element-view subslice over a known extent",
    "the subslice observation and its extent equation where the schema declares one",
    &[
        "fact:structural-effect-observation",
        "fact:element-extent-length",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_SE_ELEMENT_EXTENT,
        OP_FACTS,
        OP_FACTS_ELEMENT_EXTENT,
        VAL_OPS,
        VAL_ELEMENTS_SUBSLICE,
    ],
);
static OP_ESTABLISH_ZEROED_STRUCTURAL: TrustedSurfaceEntry = entry(
    "operation:establish-zeroed-structural",
    "a validated zeroed structural establishment",
    "no proposition facts: the total validation judgment carries the authority",
    &["formation:operation-validation"],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_ZEROED_STRUCTURAL,
        OP_FACTS,
        VAL_OPS,
        VAL_UNIT_OPERATION,
    ],
);
static OP_ESTABLISH_TRIVIAL_AFFINE_LOCAL: TrustedSurfaceEntry = entry(
    "operation:establish-trivial-affine-local",
    "a validated trivial-affine local establishment",
    "the affine establishment observation; no scalar equation is published",
    EFFECT_DEPS,
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_PARTIAL_AFFINE],
);
static OP_ESTABLISH_STRUCTURAL_LOCAL: TrustedSurfaceEntry = entry(
    "operation:establish-structural-local",
    "a validated declared structural local home establishment",
    "the home establishment observation; member contents carry their own observations and no scalar equation is published",
    EFFECT_DEPS,
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_UNIT_OPERATION],
);
static OP_ESTABLISH_RECORD: TrustedSurfaceEntry = entry(
    "operation:establish-record",
    "a validated complete record establishment in declaration field order; owned bounded-byte operands are live unrestricted whole values of the exact capacity whose independently interpreted source predicates imply the member predicates, never evidence supplied by the destination declaration",
    "the record's field-establishment facts and declared obligations",
    &[
        "fact:record-establishment",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        OP_FACTS_RECORD,
        VAL_OPS,
        VAL_RECORD,
        TS_RECORD,
    ],
);
static OP_BOOLEAN_STRUCTURAL_FIELD: TrustedSurfaceEntry = entry(
    "operation:boolean-structural-field",
    "a validated Boolean structural field read whose canonical carrier composes exact record fields and in-bounds fixed-array indices to reach a record field, or one payload field of the case a final Case step names; every case hop has exact root/prefix membership in the reconstructed incoming facts, or every other declared case of that sum is refuted by a surviving membership observation of the same place whose result is known false",
    "the read observation's field-path equation for the result",
    &[
        "fact:structural-effect-observation",
        "formation:operation-validation",
        "operation:structural-case-membership",
        "fact:structural-case-arm",
        "conversion:value-equality-denotation",
        "rule:semantic-axiom",
        "rule:implication-elimination",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_RECORD,
        OP_FACTS,
        OP_FACTS_CASE_HOPS,
        VAL_OPS,
        VAL_STRUCTURAL_SCALAR,
    ],
);
static OP_INTEGER_STRUCTURAL_FIELD: TrustedSurfaceEntry = entry(
    "operation:integer-structural-field",
    "a validated integer structural field read over a referent with a declared interval, whose canonical carrier composes exact record fields and in-bounds fixed-array indices to reach a record field or one payload field of the case a final Case step names; every case hop has exact root/prefix membership in the reconstructed incoming facts, or every other declared case of that sum is refuted by a surviving membership observation of the same place whose result is known false",
    "the field-path equation plus the declared interval bounds on the read value",
    &[
        "fact:structural-effect-observation",
        "fact:integer-structural-field-read-range",
        "formation:operation-validation",
        "operation:structural-case-membership",
        "fact:structural-case-arm",
        "conversion:value-equality-denotation",
        "rule:semantic-axiom",
        "rule:implication-elimination",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        TS_RECORD,
        OP_FACTS,
        OP_FACTS_CASE_HOPS,
        VAL_OPS,
        VAL_STRUCTURAL_SCALAR,
    ],
);
static OP_PORT_WRITE: TrustedSurfaceEntry = entry(
    "operation:port-write",
    "a validated port write of a byte sequence",
    "the port-write observation; no proposition facts are published",
    EFFECT_DEPS,
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS],
);

// -- Goal-free scalar leaf rows --

static OP_INTEGER_CONSTANT: TrustedSurfaceEntry = entry(
    "operation:integer-constant",
    "a validated integer literal and declared fixed-integer result type",
    "the result-equality axiom result = literal plus the declared carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_BOOLEAN_CONSTANT: TrustedSurfaceEntry = entry(
    "operation:boolean-constant",
    "a validated Boolean literal result",
    "the result-equality axiom result = literal and its polarity implications",
    GOAL_FREE_BOOLEAN_DEPS,
    GOAL_FREE_BOOLEAN_SITES,
);
static OP_BOOLEAN_NOT: TrustedSurfaceEntry = entry(
    "operation:boolean-not",
    "a validated Boolean operand",
    "the result-equality axiom result = not operand plus polarity implications",
    GOAL_FREE_BOOLEAN_DEPS,
    GOAL_FREE_BOOLEAN_SITES,
);
static OP_BOOLEAN_EQUAL: TrustedSurfaceEntry = entry(
    "operation:boolean-equal",
    "two validated Boolean operands",
    "the result-equality axiom result = (left == right) plus polarity implications",
    GOAL_FREE_BOOLEAN_DEPS,
    GOAL_FREE_BOOLEAN_SITES,
);
static OP_INTEGER_EQUAL: TrustedSurfaceEntry = entry(
    "operation:integer-equal",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = (left == right) plus polarity implications",
    GOAL_FREE_BOOLEAN_DEPS,
    GOAL_FREE_BOOLEAN_SITES,
);
static OP_INTEGER_LESS_THAN: TrustedSurfaceEntry = entry(
    "operation:integer-less-than",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = (left < right) plus polarity implications",
    GOAL_FREE_BOOLEAN_DEPS,
    GOAL_FREE_BOOLEAN_SITES,
);
static OP_INTEGER_LESS_OR_EQUAL: TrustedSurfaceEntry = entry(
    "operation:integer-less-or-equal",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = (left <= right) plus polarity implications",
    GOAL_FREE_BOOLEAN_DEPS,
    GOAL_FREE_BOOLEAN_SITES,
);
static OP_INTEGER_BITWISE_NOT: TrustedSurfaceEntry = entry(
    "operation:integer-bitwise-not",
    "one validated fixed-integer operand",
    "the result-equality axiom result = bitwise-not operand plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_INTEGER_WIDEN: TrustedSurfaceEntry = entry(
    "operation:integer-widen",
    "one validated fixed-integer operand and a strictly wider declared result type",
    "the result-equality axiom result = widened operand plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_INTEGER_BITWISE_AND: TrustedSurfaceEntry = entry(
    "operation:integer-bitwise-and",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = left & right plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_INTEGER_BITWISE_OR: TrustedSurfaceEntry = entry(
    "operation:integer-bitwise-or",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = left | right plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_INTEGER_BITWISE_XOR: TrustedSurfaceEntry = entry(
    "operation:integer-bitwise-xor",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = left ^ right plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_WRAPPING_INTEGER_SHIFT_LEFT: TrustedSurfaceEntry = entry(
    "operation:wrapping-integer-shift-left",
    "a validated fixed-integer value and count",
    "the result-equality axiom result = value << count (mod the carrier width) plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_WRAPPING_INTEGER_SHIFT_RIGHT: TrustedSurfaceEntry = entry(
    "operation:wrapping-integer-shift-right",
    "a validated fixed-integer value and count",
    "the result-equality axiom result = value >> count (mod the carrier width) plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_WRAPPING_INTEGER_ADD: TrustedSurfaceEntry = entry(
    "operation:wrapping-integer-add",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = left + right (mod the carrier width) plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_SATURATING_INTEGER_ADD: TrustedSurfaceEntry = entry(
    "operation:saturating-integer-add",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = saturating left + right plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_WRAPPING_INTEGER_SUBTRACT: TrustedSurfaceEntry = entry(
    "operation:wrapping-integer-subtract",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = left - right (mod the carrier width) plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_SATURATING_INTEGER_SUBTRACT: TrustedSurfaceEntry = entry(
    "operation:saturating-integer-subtract",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = saturating left - right plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_WRAPPING_INTEGER_MULTIPLY: TrustedSurfaceEntry = entry(
    "operation:wrapping-integer-multiply",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = left * right (mod the carrier width) plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);
static OP_SATURATING_INTEGER_MULTIPLY: TrustedSurfaceEntry = entry(
    "operation:saturating-integer-multiply",
    "two validated same-type fixed-integer operands",
    "the result-equality axiom result = saturating left * right plus carrier bounds",
    GOAL_FREE_DEPS,
    GOAL_FREE_SITES,
);

// -- Trapping rows --

static OP_TRAPPING_INTEGER: TrustedSurfaceEntry = entry(
    "operation:trapping-integer",
    "a validated Trapping primitive: same-carrier fixed-integer operands for add, subtract, multiply, divide and remainder, a carrier value and fixed count for a shift, any fixed source for a conversion, and a fixed-integer result",
    "the normal continuation gains result = the exact primitive term; the operation is its own Trap site, covered unconditionally or by replayed certificates proving its published same-cause ceiling under pre-operation facts and the exact failure predicate: zero or signed overflow for division, the complement of canonical successful-domain predicates otherwise",
    &[
        "fact:trapping-normal-return",
        "formation:operation-validation",
        "formation:crash-entry-requirements",
    ],
    &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS, VAL_TRAPPING],
);

static OP_FLOAT_RESULT_POLICY: TrustedSurfaceEntry = entry(
    "operation:float-result-policy",
    "an already evaluated IEEE result and, for saturation, its finite-overflow inputs, all in one exact format",
    "Trapping owns a Trap site under the complement of IEEE finiteness and establishes finite unchanged payload only on normal return; saturation establishes finite-result bounds only under exact finite-input and ordered-raw-result premises, plus a nonzero divisor for division, and establishes no unconditional finiteness or raw-result equality",
    &[
        "fact:trapping-normal-return",
        "formation:operation-validation",
        "formation:crash-entry-requirements",
    ],
    &[
        VOCAB,
        FLOAT_POLICY,
        TS_ROWS,
        OP_FACTS,
        VAL_OPS,
        VAL_FLOAT_POLICY,
    ],
);

static OP_ATOMIC_ACCESS: TrustedSurfaceEntry = entry(
    "operation:atomic-access",
    "a live root with readable authority when observing and dedicated atomic-cell identity authorizing mutation even through shared access, a static path to a fixed integer or Boolean atomic leaf, source-legal orderings, leaf-typed operands, and a leaf-typed result exactly when the event observes",
    "no proposition facts: the observed prior is an unconstrained scalar snapshot; a modifying event retires potentially aliasing atomic observations while plain siblings remain stable",
    &[
        "invalidation:atomic-access",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        OP_FACTS,
        VAL_OPS,
        VAL_ATOMIC,
        TS_SHARED_MUTATION,
        TV_STORAGE_OBSERVATIONS,
        VAL_PRIMITIVE_STORAGE,
    ],
);

// -- Proof-bearing scalar rows --

static OP_INTEGER_EXACT_CAST: TrustedSurfaceEntry = entry(
    "operation:integer-exact-cast",
    "one validated fixed-integer operand and a declared target integer type with an obligation identity",
    "the canonical goal: the operand's value is representable in the target type; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_EXACT_INTEGER_SHIFT_LEFT: TrustedSurfaceEntry = entry(
    "operation:exact-integer-shift-left",
    "a validated fixed-integer value and count with an obligation identity",
    "the canonical goal: the shifted result is representable in the value's type; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_EXACT_INTEGER_SHIFT_RIGHT: TrustedSurfaceEntry = entry(
    "operation:exact-integer-shift-right",
    "a validated fixed-integer value and count with an obligation identity",
    "the canonical goal: the count is representable for the shift; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_EXACT_INTEGER_ADD: TrustedSurfaceEntry = entry(
    "operation:exact-integer-add",
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the sum is representable in the operand type; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_EXACT_INTEGER_SUBTRACT: TrustedSurfaceEntry = entry(
    "operation:exact-integer-subtract",
    "validated same-type fixed-integer operands, address minus u64 count to address, or same-model addresses to u64 distance, with an obligation identity",
    "the canonical goal: the mathematical difference is representable in the result carrier; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_EXACT_INTEGER_MULTIPLY: TrustedSurfaceEntry = entry(
    "operation:exact-integer-multiply",
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the product is representable in the operand type; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_EXACT_INTEGER_DIVIDE: TrustedSurfaceEntry = entry(
    "operation:exact-integer-divide",
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the divisor is nonzero and the quotient is exact and representable; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_EXACT_INTEGER_REMAINDER: TrustedSurfaceEntry = entry(
    "operation:exact-integer-remainder",
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the divisor is nonzero and the remainder is defined; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_WRAPPING_INTEGER_DIVIDE: TrustedSurfaceEntry = entry(
    "operation:wrapping-integer-divide",
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the divisor is nonzero; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_WRAPPING_INTEGER_REMAINDER: TrustedSurfaceEntry = entry(
    "operation:wrapping-integer-remainder",
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the divisor is nonzero; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_SATURATING_INTEGER_DIVIDE: TrustedSurfaceEntry = entry(
    "operation:saturating-integer-divide",
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the divisor is nonzero; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);
static OP_SATURATING_INTEGER_REMAINDER: TrustedSurfaceEntry = entry(
    "operation:saturating-integer-remainder",
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the divisor is nonzero; the result equation joins the axioms",
    PROOF_BEARING_DEPS,
    PROOF_BEARING_SITES,
);

// -- Exact literal reconstruction --

static OP_IEEE_FLOAT_CONSTANT: TrustedSurfaceEntry = entry(
    "operation:ieee-float-constant",
    "a validated IEEE float constant operation whose scalar result has exactly the literal's declared format",
    "reconstruction derives the result-to-literal equation from the operation's exact bits, not a producer-asserted proposition; this is payload identity retaining signed zero and NaN payloads, not IEEE equality, finiteness, or a range bound",
    &[
        "formation:operation-validation",
        "formation:float-meaning",
        "formation:proposition-context",
        "fact:semantic-axiom-roster",
    ],
    &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS, VAL_FLOAT],
);

// -- Rows producing no proposition facts --

static OP_NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD: TrustedSurfaceEntry = entry(
    "operation:nearest-ieee-float-fused-multiply-add",
    "three validated same-format IEEE operands under the declared float meaning",
    "no proposition facts: this row reconstructs no scalar result equation; validation alone carries the row",
    &["formation:operation-validation", "formation:float-meaning"],
    &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS, VAL_FLOAT],
);
static OP_IEEE_FLOAT_COMPARE: TrustedSurfaceEntry = entry(
    "operation:ieee-float-compare",
    "two validated same-format IEEE operands under the declared float meaning",
    "no proposition facts: this row reconstructs no comparison-result equation; validation alone carries the row",
    &["formation:operation-validation", "formation:float-meaning"],
    &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS, VAL_FLOAT],
);
static OP_SCALAR_REPRESENTATION_VIEW: TrustedSurfaceEntry = entry(
    "operation:scalar-representation-view",
    "one validated operand whose declared carrier and the result's declared carrier form an admitted equal-width cross-family pair (IEEE float and integer, Boolean and 8-bit integer), with a landing obligation exactly when an integer lands on bool",
    "no result equation; an integer landing on bool owes the reconstructed canonical goal 0 <= operand <= 1 against the facts before the result exists",
    &["formation:operation-validation", "owner:operation"],
    &[
        VOCAB,
        TS_ROWS,
        TS_REPRESENTATION_VIEW,
        OP_FACTS,
        VAL_OPS,
        VAL_REPRESENTATION_VIEW,
    ],
);
static OP_STORE_DYNAMIC_DESCRIPTOR: TrustedSurfaceEntry = entry(
    "operation:store-dynamic-descriptor",
    "a validated dynamic-descriptor store",
    "no proposition facts: descriptor custody is a validation judgment only",
    NO_FACT_DEPS,
    NO_FACT_SITES,
);

// -- Call-composition rows --

static OP_CALL: TrustedSurfaceEntry = entry(
    "operation:call",
    "a validated direct call to a machine with a scalar result and argument list",
    "parameter instantiations, per-clause requires obligations, and verified ensures imports join the caller frame",
    CALL_DEPS,
    CALL_SITES,
);
static OP_CALL_UNIT: TrustedSurfaceEntry = entry(
    "operation:call-unit",
    "a validated direct call to a unit-result machine, with every Case-qualified borrowed argument proved active at its original root before call invalidation",
    "parameter instantiations and requires obligations compose; no result equation exists",
    CALL_DEPS,
    CALL_SITES,
);
static OP_CALL_STRUCTURAL_SCALAR: TrustedSurfaceEntry = entry(
    "operation:call-structural-scalar",
    "a validated call with a structural argument and scalar result, with every Case-qualified borrowed argument proved active at its original root before call invalidation",
    "the structural argument's claim custody composes with parameter instantiation, requires, and ensures",
    CALL_DEPS,
    CALL_SITES,
);
static OP_CALL_DYNAMIC_SCALAR: TrustedSurfaceEntry = entry(
    "operation:call-dynamic-scalar",
    "a validated dynamic-dispatch call with a scalar result over a vetted candidate set",
    "per-candidate instantiation composes the shared requires obligations and ensures imports",
    &[
        "composition:call-instantiation",
        "composition:dynamic-dispatch",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_CALLS,
        TV_CALLS,
        TS_SHARED_MUTATION,
        TV_STORAGE_OBSERVATIONS,
        OP_FACTS,
        VAL_OPS,
        VAL_CONTRACTS,
        VAL_DYNAMIC,
    ],
);
static OP_CALL_DYNAMIC_PARAMETER_SCALAR: TrustedSurfaceEntry = entry(
    "operation:call-dynamic-parameter-scalar",
    "a validated dynamic-dispatch call taking the callee set through a parameter, scalar result",
    "per-candidate instantiation composes the shared requires obligations and ensures imports under the parameter's custody",
    &[
        "composition:call-instantiation",
        "composition:dynamic-dispatch",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_CALLS,
        TV_CALLS,
        TS_SHARED_MUTATION,
        TV_STORAGE_OBSERVATIONS,
        OP_FACTS,
        VAL_OPS,
        VAL_CONTRACTS,
        VAL_DYNAMIC,
    ],
);
static OP_CALL_DYNAMIC_UNIT: TrustedSurfaceEntry = entry(
    "operation:call-dynamic-unit",
    "a validated dynamic-dispatch call to unit-result candidates",
    "per-candidate instantiation composes the shared requires obligations; no result equation exists",
    &[
        "composition:call-instantiation",
        "composition:dynamic-dispatch",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_CALLS,
        TV_CALLS,
        TS_SHARED_MUTATION,
        TV_STORAGE_OBSERVATIONS,
        OP_FACTS,
        VAL_OPS,
        VAL_CONTRACTS,
        VAL_DYNAMIC,
    ],
);
static OP_CALL_DYNAMIC_PARAMETER_UNIT: TrustedSurfaceEntry = entry(
    "operation:call-dynamic-parameter-unit",
    "a validated dynamic-dispatch parameter call to unit-result candidates",
    "per-candidate instantiation composes the shared requires obligations under the parameter's custody",
    &[
        "composition:call-instantiation",
        "composition:dynamic-dispatch",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_CALLS,
        TV_CALLS,
        TS_SHARED_MUTATION,
        TV_STORAGE_OBSERVATIONS,
        OP_FACTS,
        VAL_OPS,
        VAL_CONTRACTS,
        VAL_DYNAMIC,
    ],
);
static OP_CALL_STRUCTURAL: TrustedSurfaceEntry = entry(
    "operation:call-structural",
    "a validated call moving a structural value with its claims into a structural-result callee",
    "claim custody, parameter instantiation, requires, and ensures compose under the structural frame",
    CALL_DEPS,
    CALL_SITES,
);
static OP_CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS: TrustedSurfaceEntry = entry(
    "operation:call-structural-with-scalar-arguments",
    "a validated structural call that also passes scalar arguments",
    "scalar parameter instantiations compose alongside the structural claim custody",
    CALL_DEPS,
    CALL_SITES,
);
static OP_BOUNDARY_CALL: TrustedSurfaceEntry = entry(
    "operation:boundary-call",
    "a validated boundary call to an external contract under the suspension plan",
    "the boundary contract's requires obligations and ensured effects compose through the declared suspension semantics",
    &[
        "composition:boundary-call",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_CALLS,
        TS_CALLS_VIEW,
        TV_CALLS,
        TS_SHARED_MUTATION,
        TV_STORAGE_OBSERVATIONS,
        OP_FACTS,
        VAL_OPS,
        VAL_CONTRACTS,
        VAL_SUSPENSION,
    ],
);

pub static ENTRIES: &[TrustedSurfaceEntry] = &[
    OP_ESTABLISH_REFERENCE,
    OP_RELEASE_REFERENCE,
    OP_ESTABLISH_PRIMITIVE_LOCAL,
    OP_PRIMITIVE_SCALAR_READ,
    OP_STRUCTURAL_CASE_MEMBERSHIP,
    OP_STRUCTURAL_LEAF_COPY,
    OP_STRUCTURAL_CASE_LEAF_COPY,
    OP_WRITE_ONLY_PRIMITIVE_STORE,
    OP_STRUCTURAL_SCALAR_FIELD_STORE,
    OP_MOVE_STRUCTURAL_FIELD,
    OP_STORE_STRUCTURAL_FIELD,
    OP_BYTE_SEQUENCE_STORE,
    OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE,
    OP_STRUCTURAL_BYTE_VIEW_FIELD_STORE,
    OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH,
    OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE,
    OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_READ,
    OP_ESTABLISH_SCALAR_CASE,
    OP_ESTABLISH_STRUCTURAL_CASE,
    OP_ESTABLISH_SCALAR_ARRAY,
    OP_ESTABLISH_BYTE_SEQUENCE_CONCATENATION,
    OP_ESTABLISH_BYTE_SEQUENCE_LITERAL,
    OP_BYTE_SEQUENCE_LENGTH,
    OP_BYTE_SEQUENCE_WRITE,
    OP_BYTE_SEQUENCE_READ,
    OP_BYTE_SEQUENCE_SUBSLICE,
    OP_BYTE_SEQUENCE_PREDICATE,
    OP_BYTE_SEQUENCE_EQUAL,
    OP_ESTABLISH_ELEMENT_VIEW,
    OP_ESTABLISH_MUTABLE_ELEMENT_VIEW,
    OP_ELEMENT_VIEW_LENGTH,
    OP_ELEMENT_VIEW_READ,
    OP_ELEMENT_VIEW_WRITE,
    OP_ELEMENT_VIEW_SUBSLICE,
    OP_ESTABLISH_TRIVIAL_AFFINE_LOCAL,
    OP_ESTABLISH_STRUCTURAL_LOCAL,
    OP_ESTABLISH_RECORD,
    OP_STORE_DYNAMIC_DESCRIPTOR,
    OP_CALL,
    OP_CALL_UNIT,
    OP_CALL_STRUCTURAL_SCALAR,
    OP_CALL_DYNAMIC_SCALAR,
    OP_CALL_DYNAMIC_PARAMETER_SCALAR,
    OP_CALL_DYNAMIC_UNIT,
    OP_CALL_DYNAMIC_PARAMETER_UNIT,
    OP_CALL_STRUCTURAL,
    OP_CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS,
    OP_BOUNDARY_CALL,
    OP_PORT_WRITE,
    OP_INTEGER_CONSTANT,
    OP_BOOLEAN_CONSTANT,
    OP_IEEE_FLOAT_CONSTANT,
    OP_IEEE_FLOAT_COMPARE,
    OP_SCALAR_REPRESENTATION_VIEW,
    OP_NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD,
    OP_BOOLEAN_STRUCTURAL_FIELD,
    OP_INTEGER_STRUCTURAL_FIELD,
    OP_BOOLEAN_NOT,
    OP_BOOLEAN_EQUAL,
    OP_INTEGER_EQUAL,
    OP_INTEGER_LESS_THAN,
    OP_INTEGER_LESS_OR_EQUAL,
    OP_INTEGER_BITWISE_NOT,
    OP_INTEGER_WIDEN,
    OP_INTEGER_EXACT_CAST,
    OP_INTEGER_BITWISE_AND,
    OP_INTEGER_BITWISE_OR,
    OP_INTEGER_BITWISE_XOR,
    OP_WRAPPING_INTEGER_SHIFT_LEFT,
    OP_WRAPPING_INTEGER_SHIFT_RIGHT,
    OP_EXACT_INTEGER_SHIFT_LEFT,
    OP_EXACT_INTEGER_SHIFT_RIGHT,
    OP_EXACT_INTEGER_ADD,
    OP_EXACT_INTEGER_SUBTRACT,
    OP_EXACT_INTEGER_MULTIPLY,
    OP_EXACT_INTEGER_DIVIDE,
    OP_EXACT_INTEGER_REMAINDER,
    OP_WRAPPING_INTEGER_DIVIDE,
    OP_WRAPPING_INTEGER_REMAINDER,
    OP_SATURATING_INTEGER_DIVIDE,
    OP_SATURATING_INTEGER_REMAINDER,
    OP_WRAPPING_INTEGER_ADD,
    OP_SATURATING_INTEGER_ADD,
    OP_WRAPPING_INTEGER_SUBTRACT,
    OP_SATURATING_INTEGER_SUBTRACT,
    OP_WRAPPING_INTEGER_MULTIPLY,
    OP_SATURATING_INTEGER_MULTIPLY,
    OP_TRAPPING_INTEGER,
    OP_FLOAT_RESULT_POLICY,
    OP_ATOMIC_ACCESS,
];

/// `OperationSemanticTag` -> ledger entry, total by construction.
pub fn operation_schema_entry(tag: OperationSemanticTag) -> &'static TrustedSurfaceEntry {
    match tag {
        OperationSemanticTag::EstablishReference => &OP_ESTABLISH_REFERENCE,
        OperationSemanticTag::ReleaseReference => &OP_RELEASE_REFERENCE,
        OperationSemanticTag::EstablishPrimitiveLocal => &OP_ESTABLISH_PRIMITIVE_LOCAL,
        OperationSemanticTag::PrimitiveScalarRead => &OP_PRIMITIVE_SCALAR_READ,
        OperationSemanticTag::StructuralCaseMembership => &OP_STRUCTURAL_CASE_MEMBERSHIP,
        OperationSemanticTag::StructuralLeafCopy => &OP_STRUCTURAL_LEAF_COPY,
        OperationSemanticTag::StructuralCaseLeafCopy => &OP_STRUCTURAL_CASE_LEAF_COPY,
        OperationSemanticTag::WriteOnlyPrimitiveStore => &OP_WRITE_ONLY_PRIMITIVE_STORE,
        OperationSemanticTag::StructuralScalarFieldStore => &OP_STRUCTURAL_SCALAR_FIELD_STORE,
        OperationSemanticTag::MoveStructuralField => &OP_MOVE_STRUCTURAL_FIELD,
        OperationSemanticTag::StoreStructuralField => &OP_STORE_STRUCTURAL_FIELD,
        OperationSemanticTag::ByteSequenceStore => &OP_BYTE_SEQUENCE_STORE,
        OperationSemanticTag::StructuralByteSequenceFieldStore => {
            &OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE
        }
        OperationSemanticTag::StructuralByteViewFieldStore => &OP_STRUCTURAL_BYTE_VIEW_FIELD_STORE,
        OperationSemanticTag::StructuralByteSequenceFieldLength => {
            &OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH
        }
        OperationSemanticTag::StructuralByteSequenceFieldByteStore => {
            &OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE
        }
        OperationSemanticTag::StructuralByteSequenceFieldRead => {
            &OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_READ
        }
        OperationSemanticTag::EstablishScalarCase => &OP_ESTABLISH_SCALAR_CASE,
        OperationSemanticTag::EstablishStructuralCase => &OP_ESTABLISH_STRUCTURAL_CASE,
        OperationSemanticTag::EstablishScalarArray => &OP_ESTABLISH_SCALAR_ARRAY,
        OperationSemanticTag::EstablishZeroedStructural => &OP_ESTABLISH_ZEROED_STRUCTURAL,
        OperationSemanticTag::EstablishStructuralLocal => &OP_ESTABLISH_STRUCTURAL_LOCAL,
        OperationSemanticTag::EstablishByteSequenceConcatenation => {
            &OP_ESTABLISH_BYTE_SEQUENCE_CONCATENATION
        }
        OperationSemanticTag::EstablishByteSequenceLiteral => &OP_ESTABLISH_BYTE_SEQUENCE_LITERAL,
        OperationSemanticTag::ByteSequenceLength => &OP_BYTE_SEQUENCE_LENGTH,
        OperationSemanticTag::ByteSequenceWrite => &OP_BYTE_SEQUENCE_WRITE,
        OperationSemanticTag::ByteSequenceRead => &OP_BYTE_SEQUENCE_READ,
        OperationSemanticTag::ByteSequenceSubslice => &OP_BYTE_SEQUENCE_SUBSLICE,
        OperationSemanticTag::ByteSequencePredicate => &OP_BYTE_SEQUENCE_PREDICATE,
        OperationSemanticTag::ByteSequenceEqual => &OP_BYTE_SEQUENCE_EQUAL,
        OperationSemanticTag::EstablishElementView => &OP_ESTABLISH_ELEMENT_VIEW,
        OperationSemanticTag::EstablishMutableElementView => &OP_ESTABLISH_MUTABLE_ELEMENT_VIEW,
        OperationSemanticTag::ElementViewLength => &OP_ELEMENT_VIEW_LENGTH,
        OperationSemanticTag::ElementViewRead => &OP_ELEMENT_VIEW_READ,
        OperationSemanticTag::ElementViewWrite => &OP_ELEMENT_VIEW_WRITE,
        OperationSemanticTag::ElementViewSubslice => &OP_ELEMENT_VIEW_SUBSLICE,
        OperationSemanticTag::EstablishTrivialAffineLocal => &OP_ESTABLISH_TRIVIAL_AFFINE_LOCAL,
        OperationSemanticTag::EstablishRecord => &OP_ESTABLISH_RECORD,
        OperationSemanticTag::StoreDynamicDescriptor => &OP_STORE_DYNAMIC_DESCRIPTOR,
        OperationSemanticTag::Call => &OP_CALL,
        OperationSemanticTag::CallUnit => &OP_CALL_UNIT,
        OperationSemanticTag::CallStructuralScalar => &OP_CALL_STRUCTURAL_SCALAR,
        OperationSemanticTag::CallDynamicScalar => &OP_CALL_DYNAMIC_SCALAR,
        OperationSemanticTag::CallDynamicParameterScalar => &OP_CALL_DYNAMIC_PARAMETER_SCALAR,
        OperationSemanticTag::CallDynamicUnit => &OP_CALL_DYNAMIC_UNIT,
        OperationSemanticTag::CallDynamicParameterUnit => &OP_CALL_DYNAMIC_PARAMETER_UNIT,
        OperationSemanticTag::CallStructural => &OP_CALL_STRUCTURAL,
        OperationSemanticTag::CallStructuralWithScalarArguments => {
            &OP_CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS
        }
        OperationSemanticTag::BoundaryCall => &OP_BOUNDARY_CALL,
        OperationSemanticTag::PortWrite => &OP_PORT_WRITE,
        OperationSemanticTag::IntegerConstant => &OP_INTEGER_CONSTANT,
        OperationSemanticTag::BooleanConstant => &OP_BOOLEAN_CONSTANT,
        OperationSemanticTag::IeeeFloatConstant => &OP_IEEE_FLOAT_CONSTANT,
        OperationSemanticTag::IeeeFloatCompare => &OP_IEEE_FLOAT_COMPARE,
        OperationSemanticTag::ScalarRepresentationView => &OP_SCALAR_REPRESENTATION_VIEW,
        OperationSemanticTag::NearestIeeeFloatFusedMultiplyAdd => {
            &OP_NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD
        }
        OperationSemanticTag::BooleanStructuralField => &OP_BOOLEAN_STRUCTURAL_FIELD,
        OperationSemanticTag::IntegerStructuralField => &OP_INTEGER_STRUCTURAL_FIELD,
        OperationSemanticTag::BooleanNot => &OP_BOOLEAN_NOT,
        OperationSemanticTag::BooleanEqual => &OP_BOOLEAN_EQUAL,
        OperationSemanticTag::IntegerEqual => &OP_INTEGER_EQUAL,
        OperationSemanticTag::IntegerLessThan => &OP_INTEGER_LESS_THAN,
        OperationSemanticTag::IntegerLessOrEqual => &OP_INTEGER_LESS_OR_EQUAL,
        OperationSemanticTag::IntegerBitwiseNot => &OP_INTEGER_BITWISE_NOT,
        OperationSemanticTag::IntegerWiden => &OP_INTEGER_WIDEN,
        OperationSemanticTag::IntegerExactCast => &OP_INTEGER_EXACT_CAST,
        OperationSemanticTag::IntegerBitwiseAnd => &OP_INTEGER_BITWISE_AND,
        OperationSemanticTag::IntegerBitwiseOr => &OP_INTEGER_BITWISE_OR,
        OperationSemanticTag::IntegerBitwiseXor => &OP_INTEGER_BITWISE_XOR,
        OperationSemanticTag::WrappingIntegerShiftLeft => &OP_WRAPPING_INTEGER_SHIFT_LEFT,
        OperationSemanticTag::WrappingIntegerShiftRight => &OP_WRAPPING_INTEGER_SHIFT_RIGHT,
        OperationSemanticTag::ExactIntegerShiftLeft => &OP_EXACT_INTEGER_SHIFT_LEFT,
        OperationSemanticTag::ExactIntegerShiftRight => &OP_EXACT_INTEGER_SHIFT_RIGHT,
        OperationSemanticTag::ExactIntegerAdd => &OP_EXACT_INTEGER_ADD,
        OperationSemanticTag::ExactIntegerSubtract => &OP_EXACT_INTEGER_SUBTRACT,
        OperationSemanticTag::ExactIntegerMultiply => &OP_EXACT_INTEGER_MULTIPLY,
        OperationSemanticTag::ExactIntegerDivide => &OP_EXACT_INTEGER_DIVIDE,
        OperationSemanticTag::ExactIntegerRemainder => &OP_EXACT_INTEGER_REMAINDER,
        OperationSemanticTag::WrappingIntegerDivide => &OP_WRAPPING_INTEGER_DIVIDE,
        OperationSemanticTag::WrappingIntegerRemainder => &OP_WRAPPING_INTEGER_REMAINDER,
        OperationSemanticTag::SaturatingIntegerDivide => &OP_SATURATING_INTEGER_DIVIDE,
        OperationSemanticTag::SaturatingIntegerRemainder => &OP_SATURATING_INTEGER_REMAINDER,
        OperationSemanticTag::WrappingIntegerAdd => &OP_WRAPPING_INTEGER_ADD,
        OperationSemanticTag::SaturatingIntegerAdd => &OP_SATURATING_INTEGER_ADD,
        OperationSemanticTag::WrappingIntegerSubtract => &OP_WRAPPING_INTEGER_SUBTRACT,
        OperationSemanticTag::SaturatingIntegerSubtract => &OP_SATURATING_INTEGER_SUBTRACT,
        OperationSemanticTag::WrappingIntegerMultiply => &OP_WRAPPING_INTEGER_MULTIPLY,
        OperationSemanticTag::SaturatingIntegerMultiply => &OP_SATURATING_INTEGER_MULTIPLY,
        OperationSemanticTag::TrappingInteger => &OP_TRAPPING_INTEGER,
        OperationSemanticTag::FloatResultPolicy => &OP_FLOAT_RESULT_POLICY,
        OperationSemanticTag::AtomicAccess => &OP_ATOMIC_ACCESS,
    }
}
