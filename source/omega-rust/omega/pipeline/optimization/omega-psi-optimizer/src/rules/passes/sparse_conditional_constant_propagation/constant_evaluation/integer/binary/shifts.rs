//! Exact and wrapping left/right shift rule definitions.

use omega_optimization_core::OptimizationSafetyClass;

use super::model::IntegerBinaryKind;

integer_binary_evaluation_rule!(
    ExactIntegerShiftLeftConstantsRule,
    b"omega.psi-rule.exact-integer-shift-left-constants.v1",
    IntegerBinaryKind::ExactShiftLeft,
    OptimizationSafetyClass::ProofCertified
);
integer_binary_evaluation_rule!(
    ExactIntegerShiftRightConstantsRule,
    b"omega.psi-rule.exact-integer-shift-right-constants.v1",
    IntegerBinaryKind::ExactShiftRight,
    OptimizationSafetyClass::ProofCertified
);
integer_binary_evaluation_rule!(
    WrappingIntegerShiftLeftConstantsRule,
    b"omega.psi-rule.wrapping-integer-shift-left-constants.v1",
    IntegerBinaryKind::WrappingShiftLeft,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_binary_evaluation_rule!(
    WrappingIntegerShiftRightConstantsRule,
    b"omega.psi-rule.wrapping-integer-shift-right-constants.v1",
    IntegerBinaryKind::WrappingShiftRight,
    OptimizationSafetyClass::ExactOperationSemantics
);
