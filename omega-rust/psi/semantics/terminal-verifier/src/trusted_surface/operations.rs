//! Operation-schema entries: one per `OperationSemanticTag`.
//!
//! Each entry records how reconstruction treats the operation: goal-free leaf
//! denotation, proof-bearing canonical obligation, structural-effect
//! observation, call composition, or no proposition facts. The exhaustive map
//! has no wildcard arm: a new operation tag fails compilation here before
//! publication.

use terminal_semantics::OperationSemanticTag;

use super::{CoveredSurface, EntryBinding, LedgerFamily, SoundnessStatus, TrustedSurfaceEntry};

const VOCAB: &str =
    "omega-rust/psi/representations/terminal-psi/src/terminal_module/control_flow/operations.rs";
const TS_ROWS: &str = "omega-rust/psi/semantics/terminal-semantics/src/semantic_rows.rs";
const TS_LEAF_SCHEMA: &str =
    "omega-rust/psi/semantics/terminal-semantics/src/scalar_leaf_schema.rs";
const TS_LEAF_SEMANTICS: &str =
    "omega-rust/psi/semantics/terminal-semantics/src/scalar_leaf_semantics.rs";
const TS_PBS: &str = "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar.rs";
const TS_PBS_GOAL: &str =
    "omega-rust/psi/semantics/terminal-semantics/src/proof_bearing_scalar/canonical_goal.rs";
const TS_SE: &str = "omega-rust/psi/semantics/terminal-semantics/src/structural_effect.rs";
const TS_SE_EXTENT: &str =
    "omega-rust/psi/semantics/terminal-semantics/src/structural_effect/byte_extent.rs";
const TS_CALLS: &str = "omega-rust/psi/semantics/terminal-semantics/src/call_composition.rs";
const TS_CALLS_VIEW: &str =
    "omega-rust/psi/semantics/terminal-semantics/src/call_composition/fixed_byte_view.rs";
const TS_RECORD: &str = "omega-rust/psi/semantics/terminal-semantics/src/record_field.rs";
const TS_SCALAR_ARRAY: &str = "omega-rust/psi/semantics/terminal-semantics/src/scalar_array.rs";
const OP_FACTS: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts.rs";
const OP_FACTS_POLARITY: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/boolean_polarity.rs";
const OP_FACTS_BYTE_EXTENT: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/byte_extent.rs";
const OP_FACTS_RECORD: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/record.rs";
const OP_FACTS_SCALAR_CASE: &str = "omega-rust/psi/semantics/terminal-verifier/src/verification/reconstruction/operation_facts/scalar_case.rs";
const TV_CALLS: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/verification/call_composition.rs";
const VAL_OPS: &str = "omega-rust/psi/semantics/terminal-verifier/src/validation/operations.rs";
const VAL_RECORD: &str = "omega-rust/psi/semantics/terminal-verifier/src/validation/record.rs";
const VAL_SCALAR_CASE: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_case.rs";
const VAL_SCALAR_ARRAY: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/scalar_array.rs";
const VAL_CASE_MEMBERSHIP: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_case_membership.rs";
const VAL_PRIMITIVE_STORAGE: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/primitive_storage.rs";
const TS_PRIMITIVE_PLACE: &str =
    "omega-rust/psi/semantics/terminal-semantics/src/primitive_place.rs";
const VAL_REFERENCES: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/references.rs";
const VAL_STRUCTURAL_OPS: &str = "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_operations/structural_paths.rs";
const VAL_BORROWED_WINDOWS: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/borrowed_windows.rs";
const VAL_STRUCTURAL_SCALAR: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_scalar_fields.rs";
const VAL_STRUCTURAL_BYTES: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_fields.rs";
const VAL_STRUCTURAL_BYTES_STORE: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_byte_sequence_store.rs";
const VAL_BYTES_LENGTH: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_length.rs";
const VAL_BYTES_READ: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_read.rs";
const VAL_BYTES_WRITE: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_write.rs";
const VAL_BYTES_SUBSLICE: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/byte_sequence_subslice.rs";
const VAL_FLOAT: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/float_meaning.rs";
const VAL_PARTIAL_AFFINE: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/partial_affine.rs";
const VAL_CONTRACTS: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/contracts.rs";
const VAL_DYNAMIC: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/dynamic_dispatch.rs";
const VAL_SUSPENSION: &str =
    "omega-rust/psi/semantics/terminal-verifier/src/validation/suspension_call_plan.rs";

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
    TV_CALLS,
    OP_FACTS,
    VAL_OPS,
    VAL_CONTRACTS,
];
const NO_FACT_SITES: &[&str] = &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS];

// -- Structural-effect observation rows --

static OP_ESTABLISH_REFERENCE: TrustedSurfaceEntry = entry(
    "operation:establish-reference",
    "a validated source place whose reference the operation establishes",
    "the reference-establishment observation; it invalidates nothing and publishes no scalar equation",
    EFFECT_DEPS,
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_REFERENCES],
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
    "a readable live root and exact canonical relevant-field/fixed-index path to initialized primitive storage",
    "the read observation's local equation where the schema declares one",
    EFFECT_DEPS,
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_PRIMITIVE_STORAGE,
        TS_PRIMITIVE_PLACE,
    ],
);
static OP_STRUCTURAL_CASE_MEMBERSHIP: TrustedSurfaceEntry = entry(
    "operation:structural-case-membership",
    "a readable live whole root and validated field/index path to the exact nominal sum case",
    "the exact root/path/case Boolean observation; no reusable current-storage equation or payload refinement",
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
static OP_WRITE_ONLY_PRIMITIVE_STORE: TrustedSurfaceEntry = entry(
    "operation:write-only-primitive-store",
    "a writable live root, canonical relevant-field/fixed-index primitive path, and exactly typed dominating scalar value",
    "the store observation; every proposition observing the destination is invalidated",
    &[
        "fact:structural-effect-observation",
        "invalidation:write-only-primitive-store",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_PRIMITIVE_STORAGE,
        TS_PRIMITIVE_PLACE,
    ],
);
static OP_WRITE_ONLY_INDEXED_PRIMITIVE_STORE: TrustedSurfaceEntry = entry(
    "operation:write-only-indexed-primitive-store",
    "a writable live root, canonical path resolving to a declared fixed array of primitive scalars, a dominating u64 index and exactly typed scalar value, and an index-within-extent obligation",
    "the store observation; the index-within-declared-extent obligation is reconstructed and every proposition observing the destination root is invalidated",
    &[
        "fact:structural-effect-observation",
        "invalidation:write-only-primitive-store",
        "owner:operation",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_PRIMITIVE_STORAGE,
        TS_PRIMITIVE_PLACE,
    ],
);
static OP_STRUCTURAL_SCALAR_FIELD_STORE: TrustedSurfaceEntry = entry(
    "operation:structural-scalar-field-store",
    "a validated scalar field store with a range obligation exactly when its destination declaration is a bounded integer",
    "the exact stored SSA value's declared bounds are required against pre-write axioms; then propositions observing the write are invalidated and a resolvable leaf publishes its stored-value equation",
    &[
        "fact:structural-effect-observation",
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
        "omega-rust/psi/semantics/terminal-verifier/src/validation/structural_scalar_fields.rs",
    ],
);
static OP_MOVE_STRUCTURAL_FIELD: TrustedSurfaceEntry = entry(
    "operation:move-structural-field",
    "a live borrowed machine-parameter root and canonical field path to a declared non-erased structural subtree with no open repair obligation over it",
    "the extraction observation; the subtree leaves its borrowed home as restoration debt until an exact-type repair store reseats it, and every proposition observing the vacated path is invalidated",
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
    ],
);
static OP_STORE_STRUCTURAL_FIELD: TrustedSurfaceEntry = entry(
    "operation:store-structural-field",
    "an open restoration-debt hole under a borrowed machine-parameter root and a consumed same-typed value reseating it",
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
    "a validated single-byte store into a byte-sequence field",
    "propositions observing the write are invalidated; no scalar equation is published",
    &[
        "fact:structural-effect-observation",
        "invalidation:structural-field-store",
        "formation:operation-validation",
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
static OP_ESTABLISH_BYTE_SEQUENCE_LITERAL: TrustedSurfaceEntry = entry(
    "operation:establish-byte-sequence-literal",
    "a validated byte-sequence literal establishment",
    "the literal's extent observation where the schema declares one",
    &[
        "fact:structural-effect-observation",
        "fact:byte-extent-length",
        "formation:operation-validation",
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
    "a validated in-bounds byte-sequence write",
    "the write observation; propositions observing the destination extent are invalidated",
    &[
        "fact:structural-effect-observation",
        "invalidation:byte-sequence-write",
        "formation:operation-validation",
    ],
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_BYTES_WRITE],
);
static OP_BYTE_SEQUENCE_READ: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-read",
    "a validated in-bounds byte-sequence read",
    "the read observation's local equation where the schema declares one",
    EFFECT_DEPS,
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_BYTES_READ],
);
static OP_BYTE_SEQUENCE_SUBSLICE: TrustedSurfaceEntry = entry(
    "operation:byte-sequence-subslice",
    "a validated byte-sequence subslice over a known extent",
    "the subslice observation and its extent equation where the schema declares one",
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
static OP_ESTABLISH_TRIVIAL_AFFINE_LOCAL: TrustedSurfaceEntry = entry(
    "operation:establish-trivial-affine-local",
    "a validated trivial-affine local establishment",
    "the affine establishment observation; no scalar equation is published",
    EFFECT_DEPS,
    &[VOCAB, TS_ROWS, TS_SE, OP_FACTS, VAL_OPS, VAL_PARTIAL_AFFINE],
);
static OP_ESTABLISH_RECORD: TrustedSurfaceEntry = entry(
    "operation:establish-record",
    "a validated record establishment over a record shape",
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
    "a validated Boolean structural field read",
    "the read observation's field-path equation for the result",
    &[
        "fact:structural-effect-observation",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
        VAL_OPS,
        VAL_STRUCTURAL_SCALAR,
    ],
);
static OP_INTEGER_STRUCTURAL_FIELD: TrustedSurfaceEntry = entry(
    "operation:integer-structural-field",
    "a validated integer structural field read over a referent with a declared interval",
    "the field-path equation plus the declared interval bounds on the read value",
    &[
        "fact:structural-effect-observation",
        "fact:integer-structural-field-read-range",
        "formation:operation-validation",
    ],
    &[
        VOCAB,
        TS_ROWS,
        TS_SE,
        OP_FACTS,
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
    "two validated same-type fixed-integer operands with an obligation identity",
    "the canonical goal: the difference is representable in the operand type; the result equation joins the axioms",
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

// -- Rows producing no proposition facts --

static OP_IEEE_FLOAT_CONSTANT: TrustedSurfaceEntry = entry(
    "operation:ieee-float-constant",
    "a validated IEEE float literal result under the declared float meaning",
    "no proposition facts: the vocabulary has no IEEE scalar term; validation alone carries the row",
    &["formation:operation-validation", "formation:float-meaning"],
    &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS, VAL_FLOAT],
);
static OP_NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD: TrustedSurfaceEntry = entry(
    "operation:nearest-ieee-float-fused-multiply-add",
    "three validated same-format IEEE operands under the declared float meaning",
    "no proposition facts: the vocabulary has no IEEE scalar term; validation alone carries the row",
    &["formation:operation-validation", "formation:float-meaning"],
    &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS, VAL_FLOAT],
);
static OP_IEEE_FLOAT_COMPARE: TrustedSurfaceEntry = entry(
    "operation:ieee-float-compare",
    "two validated same-format IEEE operands under the declared float meaning",
    "no proposition facts: the vocabulary has no IEEE scalar term; validation alone carries the row",
    &["formation:operation-validation", "formation:float-meaning"],
    &[VOCAB, TS_ROWS, OP_FACTS, VAL_OPS, VAL_FLOAT],
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
    "a validated direct call to a unit-result machine",
    "parameter instantiations and requires obligations compose; no result equation exists",
    CALL_DEPS,
    CALL_SITES,
);
static OP_CALL_STRUCTURAL_SCALAR: TrustedSurfaceEntry = entry(
    "operation:call-structural-scalar",
    "a validated call with a structural argument and scalar result",
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
    OP_WRITE_ONLY_PRIMITIVE_STORE,
    OP_WRITE_ONLY_INDEXED_PRIMITIVE_STORE,
    OP_STRUCTURAL_SCALAR_FIELD_STORE,
    OP_MOVE_STRUCTURAL_FIELD,
    OP_STORE_STRUCTURAL_FIELD,
    OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE,
    OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH,
    OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE,
    OP_ESTABLISH_SCALAR_CASE,
    OP_ESTABLISH_SCALAR_ARRAY,
    OP_ESTABLISH_BYTE_SEQUENCE_LITERAL,
    OP_BYTE_SEQUENCE_LENGTH,
    OP_BYTE_SEQUENCE_WRITE,
    OP_BYTE_SEQUENCE_READ,
    OP_BYTE_SEQUENCE_SUBSLICE,
    OP_ESTABLISH_TRIVIAL_AFFINE_LOCAL,
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
];

/// `OperationSemanticTag` -> ledger entry, total by construction.
pub fn operation_schema_entry(tag: OperationSemanticTag) -> &'static TrustedSurfaceEntry {
    match tag {
        OperationSemanticTag::EstablishReference => &OP_ESTABLISH_REFERENCE,
        OperationSemanticTag::ReleaseReference => &OP_RELEASE_REFERENCE,
        OperationSemanticTag::EstablishPrimitiveLocal => &OP_ESTABLISH_PRIMITIVE_LOCAL,
        OperationSemanticTag::PrimitiveScalarRead => &OP_PRIMITIVE_SCALAR_READ,
        OperationSemanticTag::StructuralCaseMembership => &OP_STRUCTURAL_CASE_MEMBERSHIP,
        OperationSemanticTag::WriteOnlyPrimitiveStore => &OP_WRITE_ONLY_PRIMITIVE_STORE,
        OperationSemanticTag::WriteOnlyIndexedPrimitiveStore => {
            &OP_WRITE_ONLY_INDEXED_PRIMITIVE_STORE
        }
        OperationSemanticTag::StructuralScalarFieldStore => &OP_STRUCTURAL_SCALAR_FIELD_STORE,
        OperationSemanticTag::MoveStructuralField => &OP_MOVE_STRUCTURAL_FIELD,
        OperationSemanticTag::StoreStructuralField => &OP_STORE_STRUCTURAL_FIELD,
        OperationSemanticTag::StructuralByteSequenceFieldStore => {
            &OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE
        }
        OperationSemanticTag::StructuralByteSequenceFieldLength => {
            &OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH
        }
        OperationSemanticTag::StructuralByteSequenceFieldByteStore => {
            &OP_STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE
        }
        OperationSemanticTag::EstablishScalarCase => &OP_ESTABLISH_SCALAR_CASE,
        OperationSemanticTag::EstablishScalarArray => &OP_ESTABLISH_SCALAR_ARRAY,
        OperationSemanticTag::EstablishByteSequenceLiteral => &OP_ESTABLISH_BYTE_SEQUENCE_LITERAL,
        OperationSemanticTag::ByteSequenceLength => &OP_BYTE_SEQUENCE_LENGTH,
        OperationSemanticTag::ByteSequenceWrite => &OP_BYTE_SEQUENCE_WRITE,
        OperationSemanticTag::ByteSequenceRead => &OP_BYTE_SEQUENCE_READ,
        OperationSemanticTag::ByteSequenceSubslice => &OP_BYTE_SEQUENCE_SUBSLICE,
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
    }
}
