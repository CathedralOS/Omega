use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/artifact/mod.rs",
        coordination_marker: "pub fn lower_artifact_sections",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/optimization/mod.rs",
        coordination_marker: "pub fn build_verified_psi_optimization_unit",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/provider_installation/mod.rs",
        coordination_marker: "pub fn admit_provider_installation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/mod.rs",
        coordination_marker: "pub(crate) fn lower_decoded_verified_module",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/machine.rs",
        coordination_marker: "pub(super) fn lower_machine",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/machine/operation/mod.rs",
        coordination_marker: "pub(super) fn lower_operation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/lib.rs",
        coordination_marker: "pub fn project_optimization_run",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/replay/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/replay/candidate_decisions/mod.rs",
        coordination_marker: "manifests::validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/replay/candidate_decisions/mod.rs",
        coordination_marker: "declarations::validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/replay/candidate_decisions/mod.rs",
        coordination_marker: "baseline::validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/source/mod.rs",
        coordination_marker: "pub(super) fn project_plan",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/mod.rs",
        coordination_marker: "pub fn lower_to_target_operations_with_provider_executions_and_installation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/mod.rs",
        coordination_marker: "pub fn validate_abstract_to_target_translation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/mod.rs",
        coordination_marker: "enabled_families::ENABLED_TRANSLATION_FAMILIES",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/mod.rs",
        coordination_marker: "selection::validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/selection.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/coordination/projected_qualifications/mod.rs",
        coordination_marker: "pub(super) fn reject_unsupported",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/structural_call_return/mod.rs",
        coordination_marker: "pub(crate) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/structural_call_return/local/caller/mod.rs",
        coordination_marker: "pub(crate) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/structural_call_return/local/callee.rs",
        coordination_marker: "pub(crate) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_widen_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_bitwise_not_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_bitwise_and_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_bitwise_or_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_bitwise_xor_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_saturating_integer_add_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_saturating_integer_subtract_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_saturating_integer_multiply_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_wrapping_integer_shift_left_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_wrapping_integer_shift_right_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_wrapping_integer_divide_immediate_operands/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_wrapping_integer_remainder_immediate_operands/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_saturating_integer_divide_immediate_operands/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_wrapping_integer_add_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_wrapping_integer_subtract_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_wrapping_integer_multiply_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_boolean_not_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_boolean_equal_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_equal_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_less_than_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_less_or_equal_immediate/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_integer_exact_cast_immediate_operand/mod.rs",
        coordination_marker: "grammar::reconstruct(source)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/mod.rs",
        coordination_marker: "fn reconstruct_parameter_return",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/mod.rs",
        coordination_marker: "pub(super) mod integer",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/mod.rs",
        coordination_marker: "mod unary",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/mod.rs",
        coordination_marker: "mod bitwise",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/mod.rs",
        coordination_marker: "mod arithmetic",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/mod.rs",
        coordination_marker: "mod shift",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/shift/mod.rs",
        coordination_marker: "reconstruct_wrapping_left",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/shift/mod.rs",
        coordination_marker: "reconstruct_wrapping_right",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/shift/mod.rs",
        coordination_marker: "reconstruct_exact_left",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/shift/mod.rs",
        coordination_marker: "reconstruct_exact_right",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/shift/mod.rs",
        coordination_marker: "reconstruct_wrapping_left",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/shift/mod.rs",
        coordination_marker: "reconstruct_wrapping_right",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/shift/mod.rs",
        coordination_marker: "reconstruct_exact_left",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/shift/mod.rs",
        coordination_marker: "reconstruct_exact_right",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/mod.rs",
        coordination_marker: "fn parameter(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/bitwise/mod.rs",
        coordination_marker: "fn reconstruct_bitwise_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_add,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_divide,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_multiply,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_remainder,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_subtract,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_add,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_divide,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_multiply,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_remainder,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_subtract,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_add,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_divide,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_remainder,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_subtract,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_multiply,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/comparison/mod.rs",
        coordination_marker: "fn reconstruct_equal",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/comparison/mod.rs",
        coordination_marker: "fn reconstruct_less_or_equal",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/bitwise/mod.rs",
        coordination_marker: "fn reconstruct_bitwise_and",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/bitwise/mod.rs",
        coordination_marker: "fn reconstruct_bitwise_or",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/bitwise/mod.rs",
        coordination_marker: "fn reconstruct_bitwise_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_add,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_divide,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_multiply,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_remainder,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_exact_subtract,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_add,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_divide,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_multiply,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_remainder,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_saturating_subtract,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_add,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_divide,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_remainder,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_subtract,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/arithmetic/mod.rs",
        coordination_marker: "reconstruct_wrapping_multiply,",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/bitwise/replay.rs",
        coordination_marker: "pub(super) fn reconstruct",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/mod.rs",
        coordination_marker: "mod unary",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/mod.rs",
        coordination_marker: "mod bitwise",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/mod.rs",
        coordination_marker: "mod arithmetic",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/mod.rs",
        coordination_marker: "mod shift",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/shift/mod.rs",
        coordination_marker: "WRAPPING_INTEGER_SHIFT_LEFT",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/shift/mod.rs",
        coordination_marker: "WRAPPING_INTEGER_SHIFT_RIGHT",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/shift/mod.rs",
        coordination_marker: "EXACT_INTEGER_SHIFT_LEFT",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/shift/mod.rs",
        coordination_marker: "EXACT_INTEGER_SHIFT_RIGHT",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/error/parameter/bitwise/mod.rs",
        coordination_marker: "mod schema",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/error/parameter/bitwise/mod.rs",
        coordination_marker: "mod bitwise_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/receipt/parameter/bitwise/mod.rs",
        coordination_marker: "mod bitwise_or",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/receipt/parameter/bitwise/mod.rs",
        coordination_marker: "mod bitwise_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/error/mod.rs",
        coordination_marker: "AbstractToTargetTranslationFamilyError",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/receipt/mod.rs",
        coordination_marker: "AbstractToTargetFunctionTranslationReceipt",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/function/mod.rs",
        coordination_marker: "pub(super) fn lower_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/mod.rs",
        coordination_marker: "pub(crate) fn lower_scalar_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/straight_line/mod.rs",
        coordination_marker: "pub(super) fn lower_straight_line",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_scalar/mod.rs",
        coordination_marker: "pub(super) fn lower_conditional_scalar_operation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/structural/mod.rs",
        coordination_marker: "pub(super) fn lower_structural_function",
    },
];
