//! Reconstructed-obligation-owner, terminator-fact, and reconstructed
//! fact-kind entries.
//!
//! The two exhaustive maps below have no wildcard arm: adding an accepted
//! `ReconstructedTerminalObligationOwner` or `Terminator` variant fails
//! compilation here before publication.

use terminal_psi::Terminator;

use crate::ReconstructedTerminalObligationOwner;

use super::{CoveredSurface, EntryBinding, LedgerFamily, SoundnessStatus, TrustedSurfaceEntry};

const RECONSTRUCTION: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction.rs";
const MACHINE_FLOW: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_flow.rs";
const MACHINE_CONTEXT: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/machine_context.rs";
const PATH_FACTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts.rs";
const PATH_FACTS_CONDITIONS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/conditions.rs";
const PATH_FACTS_DISCRETE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/discrete.rs";
const PATH_FACTS_TRANSPORT: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/path_facts/transport.rs";
const OPERATION_FACTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts.rs";
const OP_FACTS_FLOAT_FIELDS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/float_fields.rs";
const OP_FACTS_PRIMITIVE_ELEMENTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/primitive_elements.rs";
const OP_FACTS_POLARITY: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/boolean_polarity.rs";
const OP_FACTS_BYTE_EXTENT: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/byte_extent.rs";
const OP_FACTS_RECORD: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/record.rs";
const OP_FACTS_SCALAR_CASE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/scalar_case.rs";
const OP_FACTS_STRUCTURAL_CASE: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/structural_case.rs";
const FIELD_REQUIREMENTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_field_requirements.rs";
const TERMINATOR_FACTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/terminator_facts.rs";
const CRASH_FIELD_ORIGINS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_field_origins.rs";
const SHARED_MUTATION: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/shared_mutation.rs";
const CRASH_PATHS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_paths.rs";
const BLOCK_INVARIANTS: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/scalar_block_invariants.rs";
const SUBSTITUTION: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/substitution.rs";
const CALL_COMPOSITION: &str = "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/call_composition.rs";
const TERMINATION: &str = "omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/termination.rs";

const TRUSTED: SoundnessStatus = SoundnessStatus::ExplicitlyTrusted {
    root: "root:rust-reference-verifier",
    rationale: "reconstruction rule witnessed by the verifier's reconstruction and rejection corpus; no lower-rung derivation discharges it",
};

const fn owner() -> EntryBinding {
    EntryBinding::DispatchOn(CoveredSurface::ObligationOwners)
}

const fn terminator() -> EntryBinding {
    EntryBinding::DispatchOn(CoveredSurface::Terminators)
}

const PROCEDURAL: EntryBinding = EntryBinding::Procedural;

// -- Reconstructed obligation owners --

static OWNER_SCALAR_BLOCK_INVARIANT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "owner:scalar-block-invariant",
    family: LedgerFamily::ObligationOwner,
    binding: owner(),
    premises: "a validated scalar block invariant declaration on a machine header block and one incoming edge",
    conclusion: "a derivable obligation whose proposition is the declared invariant instantiated with that exact edge's scalar arguments and canonical structural root/projection arguments; structural substitution precedes scalar substitution so inserted erased arguments retain predecessor scope; an observed structural argument without a canonical projection rejects rather than admitting the header hypothesis",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "fact:header-invariant-members",
        "fact:branch-condition",
        "fact:branch-condition-transport",
        "scope:header-edge-arrival",
    ],
    implementation: &[BLOCK_INVARIANTS, RECONSTRUCTION],
    soundness: TRUSTED,
};

static OWNER_OPERATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "owner:operation",
    family: LedgerFamily::ObligationOwner,
    binding: owner(),
    premises: "one operation whose schema declares a canonical obligation: proof-bearing scalar rows, structural-effect rows with a canonical obligation, the byte-sequence capacity bound, or a runtime-index segment in one of its structural projections",
    conclusion: "a derivable obligation whose proposition is exactly the schema's canonical goal, cited by the producer's declared obligation identity",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "fact:proof-bearing-scalar-goal",
        "fact:structural-effect-observation",
        "fact:byte-extent-length",
        "fact:runtime-index-bound",
    ],
    implementation: &[
        OPERATION_FACTS,
        RECONSTRUCTION,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs",
    ],
    soundness: TRUSTED,
};

static OWNER_CALL_REQUIRES: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "owner:call-requires",
    family: LedgerFamily::ObligationOwner,
    binding: owner(),
    premises: "a call operation with a callee contract requires clause, or a boundary call with a declaration scalar requires row, at a validated requirement position",
    conclusion: "a derivable obligation whose proposition is that requires clause instantiated at the call's exact argument and receiver substitution (for a boundary call, its scalar actuals substituted for the declaration's formal telescope)",
    dependencies: &[
        "fact:call-requires-instantiation",
        "composition:call-instantiation",
    ],
    implementation: &[CALL_COMPOSITION, RECONSTRUCTION],
    soundness: TRUSTED,
};

static OWNER_NOMINAL_CLEANUP_REQUIRES: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "owner:nominal-cleanup-requires",
    family: LedgerFamily::ObligationOwner,
    binding: owner(),
    premises: "a nominal affine cleanup action on a return terminator whose cleanup machine has a requires clause at a validated position",
    conclusion: "a derivable obligation whose proposition is the cleanup target's requires clause under the cleanup receiver substitution",
    dependencies: &["scope:place-substitution"],
    implementation: &[TERMINATOR_FACTS, RECONSTRUCTION],
    soundness: TRUSTED,
};

static OWNER_CONTRACT_ENSURES: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "owner:contract-ensures",
    family: LedgerFamily::ObligationOwner,
    binding: owner(),
    premises: "a machine contract ensures clause at a validated clause position and the exit facts reconstructed at a return terminator",
    conclusion: "a derivable obligation whose proposition is the ensures clause under the machine's result substitution; shared exit facts and the disjunction of every return's residual facts require a certificate valid for all alternatives",
    dependencies: &["fact:return-result-binding", "scope:place-substitution"],
    implementation: &[RECONSTRUCTION, MACHINE_FLOW],
    soundness: TRUSTED,
};

// -- Terminator fact rules --

static TERMINATOR_JUMP: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:jump",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated jump whose edge is not an ignored backedge, with the current path axioms",
    conclusion: "the successor receives the axioms plus the target-parameter bindings and the established-fact restatements under the target's bound structural formals; an ignored backedge contributes no arrival set",
    dependencies: &["fact:successor-parameter-binding", "scope:iteration-cut"],
    implementation: &[TERMINATOR_FACTS, PATH_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_CONDITIONAL: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:conditional",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated conditional on a Boolean value with both successor edges",
    conclusion: "each arm receives the axioms, its parameter bindings, the established-fact restatements under the target's bound structural formals, and the selected branch's truth fact; private crash reconstruction also retains the exact SSA truth",
    dependencies: &[
        "fact:branch-condition",
        "fact:branch-condition-transport",
        "fact:successor-parameter-binding",
        "scope:iteration-cut",
    ],
    implementation: &[
        TERMINATOR_FACTS,
        PATH_FACTS,
        PATH_FACTS_CONDITIONS,
        TERMINATION,
    ],
    soundness: TRUSTED,
};

static TERMINATOR_STRUCTURAL_CASE: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:structural-case",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated structural case over a source place with payload fields bound positionally to target parameters",
    conclusion: "each arm receives the case-membership fact, payload-field path equations for Boolean and integer fields, and the copied payload's declared bounds",
    dependencies: &["fact:structural-case-arm", "scope:iteration-cut"],
    implementation: &[TERMINATOR_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_RETURN: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:return",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated scalar return with a value and nominal cleanup actions",
    conclusion: "the exit set gains the result-equals-value equation and one cleanup-requires obligation per nominal cleanup requires clause",
    dependencies: &[
        "scope:borrowed-storage-observations",
        "fact:return-result-binding",
        "owner:nominal-cleanup-requires",
    ],
    implementation: &[TERMINATOR_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_RETURN_UNIT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:return-unit",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated unit return with trivial affine discards",
    conclusion: "the exit set carries incoming axioms plus exact current-storage facts rejoined to invocation parameters",
    dependencies: &[
        "scope:borrowed-storage-observations",
        "fact:semantic-axiom-roster",
    ],
    implementation: &[TERMINATOR_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_RETURN_UNIT_PARTIAL_AFFINE: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:return-unit-partial-affine",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated unit return with trivial and residual affine discards",
    conclusion: "the exit set carries incoming axioms plus exact current-storage facts rejoined to invocation parameters",
    dependencies: &[
        "scope:borrowed-storage-observations",
        "fact:semantic-axiom-roster",
    ],
    implementation: &[TERMINATOR_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_RETURN_UNIT_NOMINAL_AFFINE: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:return-unit-nominal-affine",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated unit return whose nominal affine cleanups each target a cleanup machine with requires clauses",
    conclusion: "the exit set carries the incoming axioms and one cleanup-requires obligation per clause under the cleanup receiver substitution",
    dependencies: &[
        "scope:borrowed-storage-observations",
        "owner:nominal-cleanup-requires",
    ],
    implementation: &[TERMINATOR_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_RETURN_STRUCTURAL: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:return-structural",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated structural return of a source place with its ordered live claims",
    conclusion: "the exit set gains the axioms rewritten through the result-place substitution plus the returned claims' content-identity reshuffle propositions",
    dependencies: &[
        "scope:borrowed-storage-observations",
        "scope:place-substitution",
        "fact:return-result-binding",
    ],
    implementation: &[TERMINATOR_FACTS, SUBSTITUTION, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_CRASH: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:crash",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated crash terminator with its reconstructed axiom set; producer-supplied site guards are never imported",
    conclusion: "a crash-site fact bundle retaining only entry-meaning facts (all facts discarded for ranked machines); it establishes no normal-return guarantee",
    dependencies: &["fact:crash-site-retention", "scope:crash-origin-filter"],
    implementation: &[
        TERMINATOR_FACTS,
        CRASH_FIELD_ORIGINS,
        SHARED_MUTATION,
        CRASH_PATHS,
        TERMINATION,
    ],
    soundness: TRUSTED,
};

// -- Reconstructed fact kinds --

static FACT_SEMANTIC_AXIOM_ROSTER: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:semantic-axiom-roster",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "the machine's operations, edges, and terminators traversed in dominance order from its entry parameters",
    conclusion: "the ordered deduplicated proposition set each obligation's assumptions and SemanticAxiom indices cite; hashing may deduplicate but never decides equality",
    dependencies: &["scope:dominance-order", "formation:proposition-context"],
    implementation: &[RECONSTRUCTION, MACHINE_FLOW, MACHINE_CONTEXT],
    soundness: TRUSTED,
};

static FACT_GOAL_FREE_SCALAR_RESULT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:goal-free-scalar-result",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a goal-free scalar leaf row with its declared result and operand shapes over validated value types",
    conclusion: "the result-equality axiom naming the leaf denotation term",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_leaf_schema.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_leaf_semantics.rs",
    ],
    soundness: TRUSTED,
};

static FACT_BOOLEAN_POLARITY: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:boolean-polarity-implications",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a goal-free leaf producing a Boolean denotation under ordinary proof-obligation reconstruction",
    conclusion: "the truth and falsity polarity implications of the result equation, each emitted only after its canonical certificate is accepted by the proof checker; private crash reconstruction omits them",
    dependencies: &[
        "fact:goal-free-scalar-result",
        "rule:semantic-axiom",
        "rule:assumption",
        "rule:predicate-denotation",
        "rule:equality-transitivity",
        "rule:implication-introduction",
        "formation:mathematical-core",
    ],
    implementation: &[
        OP_FACTS_POLARITY,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_leaf_schema.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/scalar_leaf_semantics.rs",
    ],
    soundness: SoundnessStatus::Proved {
        evidence: "boolean_polarity::implications is a total certifying procedure: every emitted implication carries a fixed-shape derivation (implication introduction over equality transitivity of the result-equation axiom and the predicate-denotation conversion of the implication's own normalized premise) accepted by proof-admission's certificate checker before the fact may join the premise roster; checker rejection fails generation closed",
    },
};

static FACT_SCALAR_CARRIER_BOUNDS: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:scalar-carrier-bounds",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a successor parameter, payload copy, or field read with independently established declared scalar validity",
    conclusion: "the exact instantiated declaration predicate, or its conjunction members, transported to the scalar snapshot only after an assumption or conjunction-elimination certificate is accepted",
    dependencies: &[
        "formation:structural-scalar-fields",
        "rule:assumption",
        "rule:conjunction-elimination",
        "formation:mathematical-core",
    ],
    implementation: &[
        PATH_FACTS_DISCRETE,
        PATH_FACTS,
        TERMINATOR_FACTS,
        OPERATION_FACTS,
        FIELD_REQUIREMENTS,
        "omega-rust/psi/foundation/semantic-vocabulary/src/scalar_refinement.rs",
        "omega-rust/psi/foundation/semantic-vocabulary/src/proposition/value_ids/remap.rs",
    ],
    soundness: SoundnessStatus::Proved {
        evidence: "declared_scalar_field_facts instantiates the closed declaration at the observed SSA value and rechecks assumption or conjunction-elimination certificates before emitting facts. This certifies transport only: storage validity remains the calling row's independently established premise, supplied by checked construction, replacement and entry admission. Rejected certificates emit no fact",
    },
};

static FACT_RECORD_ESTABLISHMENT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:record-establishment",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an EstablishRecord operation over a validated record shape with exactly the relevant field initializers in declaration order and original identity; erased declarations provide no runtime operands",
    conclusion: "the record's field-establishment facts and any declared field obligations; IEEE scalar initializers and copied child fields retain exact-format bit-identity equations, including NaN payloads and signed zero, never IEEE equality or finiteness",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        OP_FACTS_RECORD,
        FIELD_REQUIREMENTS,
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/record_field.rs",
    ],
    soundness: TRUSTED,
};

static FACT_SCALAR_CASE_ESTABLISHMENT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:scalar-case-establishment",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an EstablishScalarCase operation over a validated scalar case",
    conclusion: "the case's membership and payload facts and its declared obligations",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        OP_FACTS_SCALAR_CASE,
        FIELD_REQUIREMENTS,
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/scalar/case.rs",
    ],
    soundness: TRUSTED,
};

static FACT_STRUCTURAL_CASE_ESTABLISHMENT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:structural-case-establishment",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an EstablishStructuralCase operation over a validated structural case",
    conclusion: "the case's membership and payload facts, each mixed common field's value fact at its root-level field path, and its declared obligations",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        OP_FACTS_STRUCTURAL_CASE,
        FIELD_REQUIREMENTS,
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/case.rs",
    ],
    soundness: TRUSTED,
};

static FACT_BYTE_EXTENT_LENGTH: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:byte-extent-length",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a validated byte-sequence length observation of a borrowed machine or block parameter, or a byte-sequence operation or structural field whose extent is independently known, including a view established from exact fixed-u8 array backing, inline or a bounded reference, with validated element type and readable backing",
    conclusion: "the observation equals that exact borrowed parameter's own ViewExtent, never another descriptor's extent or an assumed capacity; independent producer/backing/arrival evidence additionally supplies exact lengths; declaration-derived upper bounds on bounded-owned fields, whole formals and exact completed ordinary owned-byte call result snapshots are never equality with capacity; callee construction capacity, result custody and dominance remain checked, as do owned concatenation and byte-field replacement capacity obligations",
    dependencies: &[
        "fact:structural-effect-observation",
        "formation:operation-validation",
        "formation:structural-result-contracts",
        "scope:dominance-order",
    ],
    implementation: &[
        OP_FACTS_BYTE_EXTENT,
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/element_extent.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/element_extent.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/collection_backing.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_extent.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_concatenation.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
    ],
    soundness: TRUSTED,
};

static FACT_BYTE_FIELD_LITERAL_READ_RANGE: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:byte-field-literal-read-range",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a validated indexed byte-field read whose exact field was replaced from a unique nonempty literal, with that content unchanged on every path from replacement to read, including backedges",
    conclusion: "the read's immutable u8 result lies between the literal's minimum and maximum octets; its independent index/length obligation remains required",
    dependencies: &[
        "formation:operation-validation",
        "fact:structural-effect-observation",
    ],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/byte_sequence_fields/freshness.rs",
    ],
    soundness: TRUSTED,
};

static FACT_BYTE_READ_CLASS: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:byte-read-class",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a validated whole ByteSequenceRead with an exact u8 result, available source and current same-source length; primitive source predicates replay from exact literal octets, parameter declarations or concatenation inputs, while a potentially mutated owned result retains only its declared predicates",
    conclusion: "the immutable read result is at least 1 for no-NUL and at most 127 for ASCII-only; UTF-8 alone supplies no byte-class bound; these facts enter only after the read's independent index obligation and describe the scalar snapshot, not later source contents",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "fact:structural-effect-observation",
        "formation:byte-sequence-operations",
    ],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/byte_sequence/read.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/byte_sequence_predicates.rs",
        "omega-rust/psi/foundation/semantic-vocabulary/src/byte_predicates.rs",
    ],
    soundness: TRUSTED,
};

static FACT_ELEMENT_EXTENT_LENGTH: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:element-extent-length",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a validated element-view length observation of a machine or block parameter, exact establishment or subslice result; relating a block parameter to a preceding observation additionally requires every arrival to trace unchanged to that same-family descriptor or producer, with mutable freshness under every intermediate name",
    conclusion: "the observation equals the exact parameter's own ViewExtent; independent producer or all-arrival evidence may additionally equate it to a preceding observation of the identical descriptor, the array's static element count, `end - start` for subslices, or `(end - start) / stride` for a normalized byte representation region, with bounds and exact tiling left as separate obligations; byte and element units are not equated",
    dependencies: &[
        "fact:structural-effect-observation",
        "formation:operation-validation",
        "scope:dominance-order",
    ],
    implementation: &[
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/representation_regions.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/element_extent.rs",
        OP_FACTS_BYTE_EXTENT,
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/length.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/element_view/subslice.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/element_extent.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/collection_backing.rs",
    ],
    soundness: TRUSTED,
};

static FACT_STRUCTURAL_EFFECT_OBSERVATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:structural-effect-observation",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a structural-effect row's validated observation of storage or an affine event",
    conclusion: "the row's canonical local equation and, where declared, its canonical derivable obligation; a validated byte predicate contributes only its Boolean result equation over the exact observed descriptor and canonical path, never a truth claim from a shared backing",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect/byte_predicate.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_effect.rs",
    ],
    soundness: TRUSTED,
};

static FACT_INTEGER_FIELD_READ_RANGE: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:scalar-structural-field-read-refinement",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a scalar structural field read over a valid referent, including a predicate-bearing payload of a proved-active case or a mixed shape's common field",
    conclusion: "the read's SSA snapshot carries the instantiated declared predicate, surviving later writes without asserting old storage contents",
    dependencies: &["fact:scalar-carrier-bounds"],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/scalar_fields.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/record_field.rs",
    ],
    soundness: SoundnessStatus::Proved {
        evidence: "the row contributes only facts from declared_scalar_field_facts, whose transport certificates are checked independently. The exact declaration and active case are validated before reading; construction, stores, and input admission preserve the storage-validity premise. Rejected transport certificates emit nothing",
    },
};

static FACT_RUNTIME_INDEX_BOUND: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:runtime-index-bound",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an operation that resolves runtime projections, whose projection carries a RuntimeIndex segment over a defined integer selector and whose segment prefix resolves to a fixed array",
    conclusion: "one derivable obligation per segment, identified by the segment's own obligation, whose proposition is `runtime_index_bound(selector, extent)` over the facts that hold before the operation; no bound is read from the segment",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/operation_facts/runtime_index.rs",
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/runtime_indexes.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/foundation.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/static_path.rs",
    ],
    soundness: TRUSTED,
};

static FACT_IEEE_PRIMITIVE_FIELD_OBSERVATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:ieee-primitive-field-observation",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a validated PrimitiveScalarRead or WriteOnlyPrimitiveStore whose root declaration and static canonical path resolve to a relevant IEEE field of exactly the SSA value's binary32 or binary64 format; the final path segment is a field, not a whole primitive local or runtime-selected element",
    conclusion: "the read result equals the current field bits, or the field equals the stored value bits after ordinary write invalidation; equality is exact interchange-bit identity preserving NaN payloads and signed zero, not IEEE comparison, FloatMeaning equality, finiteness, or a range bound",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
        "formation:proposition-context",
        "invalidation:write-only-primitive-store",
    ],
    implementation: &[
        OPERATION_FACTS,
        OP_FACTS_FLOAT_FIELDS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/primitive_place.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/primitive_storage.rs",
    ],
    soundness: TRUSTED,
};

static FACT_PRIMITIVE_ELEMENT_OBSERVATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:primitive-element-observation",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a validated PrimitiveScalarRead or WriteOnlyPrimitiveStore whose root declaration and static canonical path end at a fixed index selecting, within bounds, one element of a fixed array whose element shape is the SSA value's exact primitive scalar type; no runtime-selected segment and no record field leaf",
    conclusion: "the read result equals the current element, or the element equals the stored value after ordinary write invalidation; the element term uses the same canonical path contract field terms name, and an IEEE element keeps interchange-bit identity rather than IEEE equality",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
        "formation:proposition-context",
        "invalidation:write-only-primitive-store",
    ],
    implementation: &[
        OPERATION_FACTS,
        OP_FACTS_PRIMITIVE_ELEMENTS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/primitive_place.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/primitive_storage.rs",
    ],
    soundness: TRUSTED,
};

static FACT_FIELD_STORE_LEAF_EQUATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:field-store-leaf-equation",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a StructuralScalarFieldStore whose canonical write path resolves to an exact relevant leaf over a Boolean, integer, or same-format IEEE field",
    conclusion: "after write invalidation, the leaf-path-equals-stored-value equation; for IEEE fields the equation denotes exact bit identity preserving NaN payloads and signed zero, not IEEE comparison; an unresolvable path forgets the root instead",
    dependencies: &[
        "invalidation:structural-field-store",
        "formation:operation-validation",
    ],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/structural/operations/structural_paths.rs",
    ],
    soundness: TRUSTED,
};

static FACT_BORROWED_STORAGE_RESTORATION_DEBT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:borrowed-storage-restoration-debt",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an exact canonical field hole under an eligible storage carrier opened by MoveStructuralField and closed only by a same-typed StoreStructuralField at the identical carrier and path; a root window's hole is the empty path of an exclusive mutable loan, opened and repaired at the referent's exact type; borrowed block carriers inherit authority from independently reconstructed origins, and interference resolves aliases on both the observation and debt",
    conclusion: "the verification-time debt: every non-crash exit, join, edge transfer, and operation touching the open subtree is replayed against the hole set; a producer assertion that the owner is restored is not evidence",
    dependencies: &["formation:operation-validation"],
    implementation: &[
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/borrowed_windows.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_window.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/frontier.rs",
    ],
    soundness: TRUSTED,
};

static FACT_PROOF_BEARING_SCALAR_GOAL: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:proof-bearing-scalar-goal",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a proof-bearing scalar leaf row with declared obligation identity and operand types",
    conclusion: "the canonical goal proposition: exact-arithmetic representability, defined division, nonzero divisor, or shift-count representability; the producer cannot choose a different sufficient proposition",
    dependencies: &["formation:operation-validation"],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/semantic_rows.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/semantics/src/proof_bearing_scalar/canonical_goal.rs",
    ],
    soundness: TRUSTED,
};

static FACT_CALL_PARAMETER_INSTANTIATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:call-parameter-instantiation",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a call operation's callee signature parameters and the caller's argument values under the validated substitution",
    conclusion: "equalities binding each callee parameter to its caller argument at the callee-entry frame",
    dependencies: &["composition:call-instantiation"],
    implementation: &[CALL_COMPOSITION],
    soundness: TRUSTED,
};

static FACT_CALL_REQUIRES_INSTANTIATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:call-requires-instantiation",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a call's callee requires clauses, or a boundary call's declaration scalar requires rows, at validated positions",
    conclusion: "one derivable CallRequires obligation per clause or row, instantiated at the call substitution",
    dependencies: &["composition:call-instantiation"],
    implementation: &[CALL_COMPOSITION],
    soundness: TRUSTED,
};

static FACT_CALL_ENSURES_IMPORT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:call-ensures-import",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a call to a machine whose ensures contract is verified, or a boundary declaration with validated scalar normal-return guarantees whose installed checked provider must supply those guarantees, and an exact instantiated result frame",
    conclusion: "the callee's ensures propositions rewritten through exact input and result substitution join the caller's normal-return axiom set after mutable observation invalidation; conditional guarantees retain their premises",
    dependencies: &["composition:call-instantiation", "scope:call-frame-rewrite"],
    implementation: &[CALL_COMPOSITION],
    soundness: TRUSTED,
};

static FACT_CONTENT_PARTITION_COMPOSITION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:content-partition-composition",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a machine's declared content-partition compositions produced by a call operation",
    conclusion: "each composition's inferred proposition joins the axiom set after the call",
    dependencies: &["composition:call-instantiation"],
    implementation: &[OPERATION_FACTS, CALL_COMPOSITION],
    soundness: TRUSTED,
};

static FACT_SUCCESSOR_PARAMETER_BINDING: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:successor-parameter-binding",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a successor block's declared parameters and the edge's argument values, plus the edge's whole structural actuals when the target declares structural parameters",
    conclusion: "equalities binding each target parameter to its argument term, plus licensed premise introductions rather than a failed module: established-fact restatements under the target's bound structural formals — only whole actuals join the place substitution, and no transport certificate covers a place rewrite yet — and any established-fact restatement whose fixed-shape transport certificate the checker rejects; every accepted rewrite is discharged under fact:successor-path-transport",
    dependencies: &["scope:successor-substitution", "scope:place-substitution"],
    implementation: &[PATH_FACTS, PATH_FACTS_TRANSPORT],
    soundness: TRUSTED,
};

static FACT_SUCCESSOR_PATH_TRANSPORT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:successor-path-transport",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an established roster fact mentioning one of the edge's argument values, together with that edge's parameter-binding equalities already pushed into the same roster",
    conclusion: "the source fact with each substituted argument restated to its target parameter, deduplicated deterministically and emitted only after a fixed-shape certificate is accepted: the source fact cited as one semantic axiom and the edge's binding equalities cited as semantic axioms in roster order, checked against exactly those cited rows, so the checker re-decides that source and restatement carry the same denotation",
    dependencies: &[
        "fact:successor-parameter-binding",
        "rule:value-equality-transport",
        "rule:semantic-axiom",
        "scope:successor-substitution",
        "formation:mathematical-core",
    ],
    implementation: &[PATH_FACTS, PATH_FACTS_TRANSPORT],
    soundness: SoundnessStatus::Proved {
        evidence: "bind_successor_axioms and append_successor_fact build a fixed-shape ValueEqualityTransport certificate for every restated roster fact and call proof-admission's certificate checker before classifying the emission; only an accepted certificate marks the emission under this entry — a rejected certificate leaves it under fact:successor-parameter-binding's licensed premise introductions and never fails the module",
    },
};

static FACT_BRANCH_CONDITION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:branch-condition",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a conditional's Boolean condition and the reconstructed axiom set at the terminator",
    conclusion: "the selected arm's condition fact where the emission is a licensed premise introduction the fixed-shape transport certificate does not re-derive: the literal-adjacency strengthening of a fixed-carrier disequality, an equal-terms unsatisfiable arm's falsehood, or a boundary truth whose walk consulted a backward-only edge; every other emission is discharged under fact:branch-condition-transport, and the fact's parameter-restated copy on the edge follows the successor rewrite discipline under fact:successor-path-transport",
    dependencies: &["fact:boolean-polarity-implications"],
    implementation: &[PATH_FACTS, PATH_FACTS_CONDITIONS],
    soundness: TRUSTED,
};

static FACT_BRANCH_CONDITION_TRANSPORT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:branch-condition-transport",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a conditional's Boolean condition, the selected arm's polarity, and the reconstructed axiom roster's own value equations",
    conclusion: "the selected arm's denotation-recognized condition fact, emitted only after its certificate is accepted: the arm's truth premise is assumption zero, every SSA-value roster equation defining a value reachable from the premise and conclusion through the right sides of the cited equations is a cited semantic axiom in newest-first order — rows defining unreachable values cannot influence the checked relation, and when no reachable value has a defining row the newest row mentioning a premise or conclusion value is the single rewrite-free citation — the checker receives exactly the cited rows, and the emitted fact is exactly what that premise transports to",
    dependencies: &[
        "fact:branch-condition",
        "rule:value-equality-transport",
        "rule:semantic-axiom",
        "rule:assumption",
        "formation:mathematical-core",
    ],
    implementation: &[PATH_FACTS_CONDITIONS],
    soundness: SoundnessStatus::Proved {
        evidence: "condition_fact builds a fixed-shape ValueEqualityTransport certificate for every reconstructed arm fact and calls proof-admission's certificate checker before classifying the emission; only an accepted certificate marks the fact under this entry — a rejected certificate leaves the emission under fact:branch-condition's licensed premise introductions and never fails the module",
    },
};

static FACT_HEADER_INVARIANT_MEMBERS: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:header-invariant-members",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a validated scalar block invariant declaration on a machine header block",
    conclusion: "each conjunction member proposition joining the header's arrival roster, emitted only after its fixed-shape certificate is accepted: the declared predicate is assumption zero and the member is eliminated along its exact conjunct-index path, so the checker re-decides membership of the declaration itself; alternatives and implications are never split since their members are not independently established",
    dependencies: &[
        "scope:header-edge-arrival",
        "rule:assumption",
        "rule:conjunction-elimination",
        "formation:scalar-block-invariants",
        "formation:mathematical-core",
    ],
    implementation: &[BLOCK_INVARIANTS],
    soundness: SoundnessStatus::Proved {
        evidence: "certified_members walks each declared predicate's conjunction tree recording the conjunct-index path, and member_certified builds a fixed-shape Assumption-plus-chained-ConjunctionElimination certificate that proof-admission's certificate checker re-decides before the emission is classified; only an accepted certificate marks the fact under this entry — a rejected certificate leaves the emission under scope:header-edge-arrival's licensed premise introductions and never fails the module",
    },
};

static FACT_STRUCTURAL_CASE_ARM: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:structural-case-arm",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a structural-case arm's selected case and payload fields bound to target parameters",
    conclusion: "the StructuralCaseMembership fact, field-path equations for Boolean and integer payloads, and checked declared predicates on the copied scalar snapshots",
    dependencies: &["fact:scalar-carrier-bounds"],
    implementation: &[TERMINATOR_FACTS],
    soundness: TRUSTED,
};

static FACT_RETURN_RESULT_BINDING: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:return-result-binding",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a return's value or structural source and the machine result frame",
    conclusion: "the result-equals-value equation or the place-substituted axiom set with returned-claim reshuffle propositions",
    dependencies: &["scope:place-substitution"],
    implementation: &[TERMINATOR_FACTS, SUBSTITUTION],
    soundness: TRUSTED,
};

static FACT_TRAPPING_NORMAL_RETURN: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:trapping-normal-return",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a validated Trapping integer primitive or IEEE result policy whose failure predicate is reconstructed from its operand and result carriers",
    conclusion: "only the normal continuation gains the integer primitive's exact result equation, or the IEEE policy's finite-result bounds and equality to its raw payload; crash coverage is checked under pre-operation facts before these normal-return facts become available, without an Exact safety obligation",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/trapping_integer.rs",
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/float_result_policy.rs",
    ],
    soundness: TRUSTED,
};

static FACT_CRASH_SITE_RETENTION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:crash-site-retention",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "the axiom set reconstructed before a crash terminator or a crash-capable operation",
    conclusion: "only facts whose observed storage keeps entry meaning survive; cyclic coverage retains entry-only facts, while call-time refutation excludes mutable entry observations; primitive failure predicates are independently reconstructed before their normal-return equations, and producer site guards or ceilings are never imported",
    dependencies: &["scope:crash-origin-filter"],
    implementation: &[
        CRASH_FIELD_ORIGINS,
        SHARED_MUTATION,
        CRASH_PATHS,
        TERMINATOR_FACTS,
        RECONSTRUCTION,
        "omega-rust/psi/compilation/representations/terminal-psi/verification/src/verification/reconstruction/crash_obligations.rs",
    ],
    soundness: TRUSTED,
};

pub static ENTRIES: &[TrustedSurfaceEntry] = &[
    OWNER_SCALAR_BLOCK_INVARIANT,
    OWNER_OPERATION,
    OWNER_CALL_REQUIRES,
    OWNER_NOMINAL_CLEANUP_REQUIRES,
    OWNER_CONTRACT_ENSURES,
    TERMINATOR_JUMP,
    TERMINATOR_CONDITIONAL,
    TERMINATOR_STRUCTURAL_CASE,
    TERMINATOR_RETURN,
    TERMINATOR_RETURN_UNIT,
    TERMINATOR_RETURN_UNIT_PARTIAL_AFFINE,
    TERMINATOR_RETURN_UNIT_NOMINAL_AFFINE,
    TERMINATOR_RETURN_STRUCTURAL,
    TERMINATOR_CRASH,
    FACT_SEMANTIC_AXIOM_ROSTER,
    FACT_GOAL_FREE_SCALAR_RESULT,
    FACT_BOOLEAN_POLARITY,
    FACT_SCALAR_CARRIER_BOUNDS,
    FACT_RECORD_ESTABLISHMENT,
    FACT_SCALAR_CASE_ESTABLISHMENT,
    FACT_STRUCTURAL_CASE_ESTABLISHMENT,
    FACT_BYTE_EXTENT_LENGTH,
    FACT_BYTE_FIELD_LITERAL_READ_RANGE,
    FACT_BYTE_READ_CLASS,
    FACT_ELEMENT_EXTENT_LENGTH,
    FACT_STRUCTURAL_EFFECT_OBSERVATION,
    FACT_INTEGER_FIELD_READ_RANGE,
    FACT_RUNTIME_INDEX_BOUND,
    FACT_IEEE_PRIMITIVE_FIELD_OBSERVATION,
    FACT_PRIMITIVE_ELEMENT_OBSERVATION,
    FACT_FIELD_STORE_LEAF_EQUATION,
    FACT_BORROWED_STORAGE_RESTORATION_DEBT,
    FACT_PROOF_BEARING_SCALAR_GOAL,
    FACT_CALL_PARAMETER_INSTANTIATION,
    FACT_CALL_REQUIRES_INSTANTIATION,
    FACT_CALL_ENSURES_IMPORT,
    FACT_CONTENT_PARTITION_COMPOSITION,
    FACT_SUCCESSOR_PARAMETER_BINDING,
    FACT_SUCCESSOR_PATH_TRANSPORT,
    FACT_BRANCH_CONDITION,
    FACT_BRANCH_CONDITION_TRANSPORT,
    FACT_HEADER_INVARIANT_MEMBERS,
    FACT_STRUCTURAL_CASE_ARM,
    FACT_RETURN_RESULT_BINDING,
    FACT_CRASH_SITE_RETENTION,
    FACT_TRAPPING_NORMAL_RETURN,
];

/// `ReconstructedTerminalObligationOwner` -> ledger entry, total by
/// construction.
pub fn obligation_owner_entry(
    owner: &ReconstructedTerminalObligationOwner,
) -> &'static TrustedSurfaceEntry {
    match owner {
        ReconstructedTerminalObligationOwner::ScalarBlockInvariant { .. } => {
            &OWNER_SCALAR_BLOCK_INVARIANT
        }
        ReconstructedTerminalObligationOwner::Operation { .. } => &OWNER_OPERATION,
        ReconstructedTerminalObligationOwner::CallRequires { .. } => &OWNER_CALL_REQUIRES,
        ReconstructedTerminalObligationOwner::NominalCleanupRequires { .. } => {
            &OWNER_NOMINAL_CLEANUP_REQUIRES
        }
        ReconstructedTerminalObligationOwner::ContractEnsures { .. } => &OWNER_CONTRACT_ENSURES,
    }
}

/// `Terminator` -> ledger entry, total by construction.
pub fn terminator_fact_entry(terminator: &Terminator) -> &'static TrustedSurfaceEntry {
    match terminator {
        Terminator::Jump { .. } => &TERMINATOR_JUMP,
        Terminator::Conditional { .. } => &TERMINATOR_CONDITIONAL,
        Terminator::StructuralCase { .. } => &TERMINATOR_STRUCTURAL_CASE,
        Terminator::Return { .. } => &TERMINATOR_RETURN,
        Terminator::ReturnUnit { .. } => &TERMINATOR_RETURN_UNIT,
        Terminator::ReturnUnitPartialAffine { .. } => &TERMINATOR_RETURN_UNIT_PARTIAL_AFFINE,
        Terminator::ReturnUnitNominalAffine { .. } => &TERMINATOR_RETURN_UNIT_NOMINAL_AFFINE,
        Terminator::ReturnStructural { .. } => &TERMINATOR_RETURN_STRUCTURAL,
        Terminator::Crash { .. } => &TERMINATOR_CRASH,
    }
}
