//! Reconstructed-obligation-owner, terminator-fact, and reconstructed
//! fact-kind entries.
//!
//! The two exhaustive maps below have no wildcard arm: adding an accepted
//! `ReconstructedTerminalObligationOwner` or `Terminator` variant fails
//! compilation here before publication.

use terminal_psi::Terminator;

use crate::ReconstructedTerminalObligationOwner;

use super::{CoveredSurface, EntryBinding, LedgerFamily, SoundnessStatus, TrustedSurfaceEntry};

const RECONSTRUCTION: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction.rs";
const MACHINE_FLOW: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/machine_flow.rs";
const MACHINE_CONTEXT: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/machine_context.rs";
const PATH_FACTS: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts.rs";
const PATH_FACTS_CONDITIONS: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs";
const PATH_FACTS_DISCRETE: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/discrete.rs";
const PATH_FACTS_TRANSPORT: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/path_facts/transport.rs";
const OPERATION_FACTS: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts.rs";
const OP_FACTS_POLARITY: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/boolean_polarity.rs";
const OP_FACTS_BYTE_EXTENT: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/byte_extent.rs";
const OP_FACTS_RECORD: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/record.rs";
const OP_FACTS_SCALAR_CASE: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/scalar_case.rs";
const TERMINATOR_FACTS: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/terminator_facts.rs";
const CRASH_FIELD_ORIGINS: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_field_origins.rs";
const CRASH_PATHS: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/crash_paths.rs";
const BLOCK_INVARIANTS: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/scalar_block_invariants.rs";
const SUBSTITUTION: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/substitution.rs";
const CALL_COMPOSITION: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/call_composition.rs";
const TERMINATION: &str =
    "omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/termination.rs";

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
    conclusion: "a derivable obligation whose proposition is the declared invariant instantiated for that edge's arrival state",
    dependencies: &[
        "fact:semantic-axiom-roster",
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
    premises: "one operation whose schema declares a canonical obligation: proof-bearing scalar rows, structural-effect rows with a canonical obligation, or the byte-sequence capacity bound",
    conclusion: "a derivable obligation whose proposition is exactly the schema's canonical goal, cited by the producer's declared obligation identity",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "fact:proof-bearing-scalar-goal",
        "fact:structural-effect-observation",
        "fact:byte-extent-length",
    ],
    implementation: &[
        OPERATION_FACTS,
        RECONSTRUCTION,
        "omega-rust/psi/semantics/terminal-semantics/src/semantic_rows.rs",
    ],
    soundness: TRUSTED,
};

static OWNER_CALL_REQUIRES: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "owner:call-requires",
    family: LedgerFamily::ObligationOwner,
    binding: owner(),
    premises: "a call operation with a callee contract requires clause at a validated requirement position",
    conclusion: "a derivable obligation whose proposition is that requires clause instantiated at the call's exact argument and receiver substitution",
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
    conclusion: "a derivable obligation whose proposition is the ensures clause under the machine's result substitution, proven from the reconstructed exit facts",
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
    conclusion: "the successor receives the axioms plus the target-parameter bindings; an ignored backedge contributes no arrival set",
    dependencies: &["fact:successor-parameter-binding", "scope:iteration-cut"],
    implementation: &[TERMINATOR_FACTS, PATH_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_CONDITIONAL: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:conditional",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated conditional on a Boolean value with both successor edges",
    conclusion: "each arm receives the axioms, its parameter bindings, and the selected branch's truth fact; private crash reconstruction also retains the exact SSA truth",
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
    conclusion: "the exit set carries exactly the incoming axioms",
    dependencies: &["fact:semantic-axiom-roster"],
    implementation: &[TERMINATOR_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_RETURN_UNIT_PARTIAL_AFFINE: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:return-unit-partial-affine",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated unit return with trivial and residual affine discards",
    conclusion: "the exit set carries exactly the incoming axioms",
    dependencies: &["fact:semantic-axiom-roster"],
    implementation: &[TERMINATOR_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_RETURN_UNIT_NOMINAL_AFFINE: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:return-unit-nominal-affine",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated unit return whose nominal affine cleanups each target a cleanup machine with requires clauses",
    conclusion: "the exit set carries the incoming axioms and one cleanup-requires obligation per clause under the cleanup receiver substitution",
    dependencies: &["owner:nominal-cleanup-requires"],
    implementation: &[TERMINATOR_FACTS, TERMINATION],
    soundness: TRUSTED,
};

static TERMINATOR_RETURN_STRUCTURAL: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "terminator:return-structural",
    family: LedgerFamily::ReconstructedFactKind,
    binding: terminator(),
    premises: "a validated structural return of a source place with its ordered live claims",
    conclusion: "the exit set gains the axioms rewritten through the result-place substitution plus the returned claims' content-identity reshuffle propositions",
    dependencies: &["scope:place-substitution", "fact:return-result-binding"],
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
        "omega-rust/psi/semantics/terminal-semantics/src/semantic_rows.rs",
        "omega-rust/psi/semantics/terminal-semantics/src/scalar_leaf_schema.rs",
        "omega-rust/psi/semantics/terminal-semantics/src/scalar_leaf_semantics.rs",
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
        "omega-rust/psi/semantics/terminal-semantics/src/semantic_rows.rs",
        "omega-rust/psi/semantics/terminal-semantics/src/scalar_leaf_schema.rs",
        "omega-rust/psi/semantics/terminal-semantics/src/scalar_leaf_semantics.rs",
    ],
    soundness: SoundnessStatus::Proved {
        evidence: "boolean_polarity::implications is a total certifying procedure: every emitted implication carries a fixed-shape derivation (implication introduction over equality transitivity of the result-equation axiom and the predicate-denotation conversion of the implication's own normalized premise) accepted by proof-admission's certificate checker before the fact may join the premise roster; checker rejection fails generation closed",
    },
};

static FACT_SCALAR_CARRIER_BOUNDS: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:scalar-carrier-bounds",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a successor parameter, payload copy, or field read of a fixed integer or bounded-integer type",
    conclusion: "LessOrEqual propositions bounding the value by its type's exact minimum and maximum",
    dependencies: &["primitive:integer-carrier-bound"],
    implementation: &[
        PATH_FACTS_DISCRETE,
        PATH_FACTS,
        TERMINATOR_FACTS,
        OPERATION_FACTS,
    ],
    soundness: TRUSTED,
};

static FACT_RECORD_ESTABLISHMENT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:record-establishment",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an EstablishRecord operation over a validated record shape",
    conclusion: "the record's field-establishment facts and any declared field obligations",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        OP_FACTS_RECORD,
        OPERATION_FACTS,
        "omega-rust/psi/semantics/terminal-semantics/src/record_field.rs",
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
        OPERATION_FACTS,
        "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_case.rs",
    ],
    soundness: TRUSTED,
};

static FACT_BYTE_EXTENT_LENGTH: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:byte-extent-length",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a byte-sequence operation or structural byte-sequence field whose validated extent is known",
    conclusion: "the length equation and, for a structural byte-sequence field store, the length-at-most-capacity obligation",
    dependencies: &[
        "fact:structural-effect-observation",
        "formation:operation-validation",
    ],
    implementation: &[
        OP_FACTS_BYTE_EXTENT,
        OPERATION_FACTS,
        "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_fields.rs",
        "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/byte_extent.rs",
    ],
    soundness: TRUSTED,
};

static FACT_STRUCTURAL_EFFECT_OBSERVATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:structural-effect-observation",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a structural-effect row's validated observation of storage or an affine event",
    conclusion: "the row's canonical local equation and, where declared, its canonical derivable obligation",
    dependencies: &[
        "fact:semantic-axiom-roster",
        "formation:operation-validation",
    ],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/semantics/terminal-semantics/src/semantic_rows.rs",
        "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs",
    ],
    soundness: TRUSTED,
};

static FACT_INTEGER_FIELD_READ_RANGE: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:integer-structural-field-read-range",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an integer structural field read over a referent whose declared field interval is valid",
    conclusion: "the read's SSA value carries the declared interval as LessOrEqual bounds, surviving later writes",
    dependencies: &["fact:scalar-carrier-bounds"],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_scalar_fields.rs",
    ],
    soundness: TRUSTED,
};

static FACT_FIELD_STORE_LEAF_EQUATION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:field-store-leaf-equation",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a StructuralScalarFieldStore whose canonical write path resolves to an exact leaf over a Boolean or integer field",
    conclusion: "the leaf-path-equals-stored-value equation; an unresolvable path forgets the root instead",
    dependencies: &[
        "invalidation:structural-field-store",
        "formation:operation-validation",
    ],
    implementation: &[
        OPERATION_FACTS,
        "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_paths.rs",
    ],
    soundness: TRUSTED,
};

static FACT_BORROWED_STORAGE_RESTORATION_DEBT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:borrowed-storage-restoration-debt",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an exact canonical field hole under a borrowed machine-parameter root opened by MoveStructuralField and closed only by a same-typed StoreStructuralField at the identical path",
    conclusion: "the verification-time debt: every non-crash exit, join, edge transfer, and operation touching the open subtree is replayed against the hole set; a producer assertion that the owner is restored is not evidence",
    dependencies: &["formation:operation-validation"],
    implementation: &[
        "omega-rust/psi/semantics/terminal-verifier/src/validation/borrowed_windows.rs",
        "omega-rust/psi/semantics/terminal-verifier/src/validation/frontier.rs",
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
        "omega-rust/psi/semantics/terminal-semantics/src/semantic_rows.rs",
        "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar.rs",
        "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar/canonical_goal.rs",
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
    premises: "a call's callee requires clauses at validated positions",
    conclusion: "one derivable CallRequires obligation per clause, instantiated at the call substitution",
    dependencies: &["composition:call-instantiation"],
    implementation: &[CALL_COMPOSITION],
    soundness: TRUSTED,
};

static FACT_CALL_ENSURES_IMPORT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:call-ensures-import",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a call to a machine whose ensures contract is verified and whose result frame is instantiated",
    conclusion: "the callee's ensures propositions rewritten into the caller frame join the caller's axiom set",
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
    premises: "a successor block's declared parameters and the edge's argument values",
    conclusion: "equalities binding each target parameter to its argument term, plus any established-fact restatement whose fixed-shape transport certificate the checker rejects — a licensed premise introduction rather than a failed module; every accepted rewrite is discharged under fact:successor-path-transport",
    dependencies: &["scope:successor-substitution"],
    implementation: &[PATH_FACTS, PATH_FACTS_TRANSPORT],
    soundness: TRUSTED,
};

static FACT_SUCCESSOR_PATH_TRANSPORT: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:successor-path-transport",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "an established roster fact mentioning one of the edge's argument values, together with that edge's parameter-binding equalities already pushed into the same roster",
    conclusion: "the source fact with each substituted argument restated to its target parameter, deduplicated deterministically and emitted only after a fixed-shape certificate is accepted: the source fact cited as one semantic axiom and the edge's binding equalities cited as semantic axioms in roster order, so the checker re-decides that source and restatement carry the same denotation",
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
    conclusion: "the selected arm's denotation-recognized condition fact, emitted only after its certificate is accepted: the arm's truth premise is assumption zero, every roster equation headed by an SSA value is a cited semantic axiom in newest-first order, and the emitted fact is exactly what that premise transports to",
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

static FACT_STRUCTURAL_CASE_ARM: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:structural-case-arm",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "a structural-case arm's selected case and payload fields bound to target parameters",
    conclusion: "the StructuralCaseMembership fact, field-path equations for Boolean and integer payloads, and bounded-integer payload bounds on the copied values",
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

static FACT_CRASH_SITE_RETENTION: TrustedSurfaceEntry = TrustedSurfaceEntry {
    id: "fact:crash-site-retention",
    family: LedgerFamily::ReconstructedFactKind,
    binding: PROCEDURAL,
    premises: "the axiom set reconstructed before a crash terminator",
    conclusion: "only facts whose observed storage keeps entry meaning survive; ranked machines discard the entire set; producer site guards are never imported",
    dependencies: &["scope:crash-origin-filter"],
    implementation: &[CRASH_FIELD_ORIGINS, CRASH_PATHS, TERMINATOR_FACTS],
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
    FACT_BYTE_EXTENT_LENGTH,
    FACT_STRUCTURAL_EFFECT_OBSERVATION,
    FACT_INTEGER_FIELD_READ_RANGE,
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
    FACT_STRUCTURAL_CASE_ARM,
    FACT_RETURN_RESULT_BINDING,
    FACT_CRASH_SITE_RETENTION,
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
