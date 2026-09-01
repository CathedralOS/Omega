//! Optimizer module role: stage group.
use super::*;
pub(super) use crate::lowering::{
    lower_with_settlements_for_tests as lower_to_target_operations_with_settlements,
    structural_shape_for_tests as structural_shape,
};
pub(super) use std::collections::{BTreeMap, BTreeSet};

pub(super) use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractSuccessor, ValueBinding,
};
pub(super) use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
pub(super) use omega_installation_evidence::{
    InstalledProviderCompletionClaimSource, InstalledProviderUnitCallEvidence,
    ProviderInstallationEvidence,
};
pub(super) use omega_target::NativeTarget;
pub(super) use omega_target_operations::{
    BoundaryRealization, BoundarySettlementBinding, MachineRegister, ScalarParameterLocation,
    TargetBooleanControl, TargetBooleanExpression, TargetIntegerExpression, TargetOperation,
    TargetUnitOperation, TargetUnitScalarArgumentSource,
};
pub(super) use psi_core::{
    BlockId, BoundaryMachineId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
pub(super) use psi_terminal::{
    BoundaryMachineDeclaration, CrashCause, CrashRouteBucket, CrashRouteGuard, SemanticFingerprint,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    TerminalPsiIdentity, VocabularyMarker,
};

mod fixed_integer_scalar_abi;
mod native_boundaries;
mod parameter_translation_fixture;
mod ranked_countdown;
mod scalar;
mod structural_and_cleanup;
mod translation_validation;
mod translation_validation_boolean;
mod translation_validation_boolean_equal_parameters;
mod translation_validation_boolean_not_parameter;
mod translation_validation_boolean_parameter;
mod translation_validation_crash;
mod translation_validation_exact_integer_add_parameters;
mod translation_validation_exact_integer_divide_parameters;
mod translation_validation_exact_integer_multiply_parameters;
mod translation_validation_exact_integer_remainder_parameters;
mod translation_validation_exact_integer_subtract_parameters;
mod translation_validation_integer_bitwise_and_parameters;
mod translation_validation_integer_bitwise_not_parameter;
mod translation_validation_integer_bitwise_or_parameters;
mod translation_validation_integer_bitwise_xor_parameters;
mod translation_validation_integer_equal_parameters;
mod translation_validation_integer_exact_cast_parameter;
mod translation_validation_integer_less_or_equal_parameters;
mod translation_validation_integer_less_than_parameters;
mod translation_validation_integer_parameter;
mod translation_validation_integer_widen_parameter;
mod translation_validation_saturating_integer_add_parameters;
mod translation_validation_saturating_integer_multiply_parameters;
mod translation_validation_saturating_integer_subtract_parameters;
mod translation_validation_wrapping_integer_add_parameters;
mod translation_validation_wrapping_integer_divide_parameters;
mod translation_validation_wrapping_integer_multiply_parameters;
mod translation_validation_wrapping_integer_subtract_parameters;
mod unit_and_settlements;
mod unit_scalar_calls;
mod unit_structural_calls;

pub(super) fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
    }
}

pub(super) fn scalar_result(function: &AbstractFunction) -> AbstractResult {
    function.result.scalar().expect("fixture is scalar")
}

pub(super) fn scalar_result_mut(function: &mut AbstractFunction) -> &mut AbstractResult {
    let AbstractFunctionResult::Scalar(result) = &mut function.result else {
        panic!("fixture is scalar")
    };
    result
}
