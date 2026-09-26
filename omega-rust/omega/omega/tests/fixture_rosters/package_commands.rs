//! Corpus sources rejected by the shipped package CLI with inline diagnostics.

/// Single-file `tests/omega/fail` members whose rejection lives inside checked
/// compilation, so `omega install` reports `checked compilation failed for
/// package` on every host. One member per stage keeps the leg exercising
/// parse, resolution, typing, borrow, proof, termination, and build-time
/// evaluation rejections rather than one stage. Multi-file fixtures and
/// members whose rejection is target- or host-specific stay with the corpus.
pub(crate) const FAIL_CANARIES: &[&str] = &[
    "arithmetic/anonymous_remainder_rejected",
    "borrows/borrow_duplicate_mut",
    "comptime/fuel_exhausted_const_array_length",
    "domains/exit_ensures_unproven",
    "expressions/bool_numeric_operand_mixing_rejected",
    "modules/boundary_signature_selects_private_data",
    "ownership/assign_immutable_parameter",
    "parse/machine_clause_garbage_rejected",
    "slices/index_operator_contract_unproven",
    "termination/computed_measure_non_monotone_body",
    "traits/conformance_item_missing_member",
];
