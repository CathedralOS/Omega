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
pub(super) use omega_calling_conventions::{ValueLocation, ValueShape};
pub(super) use omega_installation_evidence::{
    InstalledProviderCompletionClaimSource, InstalledProviderUnitCallEvidence,
    ProviderInstallationEvidence,
};
pub(super) use omega_target::NativeTarget;
pub(super) use omega_target_operations::{
    BoundaryRealization, BoundarySettlementBinding, MachineRegister, ScalarParameterLocation,
    TargetBooleanControl, TargetBooleanExpression, TargetIntegerExpression, TargetOperation,
    TargetUnitOperation,
};
pub(super) use psi_core::{
    BlockId, BoundaryMachineId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
pub(super) use psi_terminal::{
    BoundaryMachineDeclaration, SemanticFingerprint, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};

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
mod translation_validation_integer_equal_parameters;
mod translation_validation_integer_less_or_equal_parameters;
mod translation_validation_integer_less_than_parameters;
mod translation_validation_integer_parameter;
mod unit_and_settlements;

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
