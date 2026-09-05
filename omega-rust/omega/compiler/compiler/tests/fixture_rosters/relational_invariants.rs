//! Fixture identities and ordered execution tables for relational invariant tests.

pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_DYNAMIC_LENGTH_COMPILE: &str =
    "dependent/relational_loop_invariant_dynamic_length_compile";
pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_REASSIGNED_INDEX_REJECTED: &str =
    "dependent/relational_loop_invariant_reassigned_index_rejected";
pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_COLLECTION_CALL_REJECTED: &str =
    "dependent/relational_loop_invariant_collection_call_rejected";
pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_STABLE_LIMIT_COMPILE: &str =
    "dependent/relational_loop_invariant_stable_limit_compile";
pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_MIXED_STRICTNESS_COMPILE: &str =
    "dependent/relational_loop_invariant_mixed_strictness_compile";
pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_LIMIT_BRIDGE_ABSENT_REJECTED: &str =
    "dependent/relational_loop_invariant_limit_bridge_absent_rejected";
pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_LIMIT_CALL_REJECTED: &str =
    "dependent/relational_loop_invariant_limit_call_rejected";
pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_LIMIT_PREHEADER_WRITE_REJECTED: &str =
    "dependent/relational_loop_invariant_limit_preheader_write_rejected";
pub const DEPENDENT_RELATIONAL_LOOP_INVARIANT_FULLY_NONSTRICT_REJECTED: &str =
    "dependent/relational_loop_invariant_fully_nonstrict_rejected";

pub const PASS_CANARIES: &[&str] = &[DEPENDENT_RELATIONAL_LOOP_INVARIANT_DYNAMIC_LENGTH_COMPILE];

pub const HEAD_FACT_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        DEPENDENT_RELATIONAL_LOOP_INVARIANT_REASSIGNED_INDEX_REJECTED,
        "reassigning the index must invalidate the relational loop fact",
    ),
    (
        DEPENDENT_RELATIONAL_LOOP_INVARIANT_COLLECTION_CALL_REJECTED,
        "a collection-overlapping call must block the relational loop fact",
    ),
];

pub const STABLE_LIMIT_PASS_CANARIES: &[&str] = &[
    DEPENDENT_RELATIONAL_LOOP_INVARIANT_STABLE_LIMIT_COMPILE,
    DEPENDENT_RELATIONAL_LOOP_INVARIANT_MIXED_STRICTNESS_COMPILE,
];

pub const STABLE_LIMIT_FAIL_CANARIES: &[&str] = &[
    DEPENDENT_RELATIONAL_LOOP_INVARIANT_LIMIT_BRIDGE_ABSENT_REJECTED,
    DEPENDENT_RELATIONAL_LOOP_INVARIANT_LIMIT_CALL_REJECTED,
    DEPENDENT_RELATIONAL_LOOP_INVARIANT_LIMIT_PREHEADER_WRITE_REJECTED,
    DEPENDENT_RELATIONAL_LOOP_INVARIANT_FULLY_NONSTRICT_REJECTED,
];
