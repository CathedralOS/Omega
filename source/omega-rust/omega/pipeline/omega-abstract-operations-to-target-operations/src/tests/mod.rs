//! Optimizer module role: stage group.
use super::*;
pub(super) use crate::lowering::{
    bind_native_callback_arguments_for_tests as bind_native_callback_arguments,
    lower_with_settlements_for_tests as lower_to_target_operations_with_settlements,
    structural_shape_for_tests as structural_shape,
    validate_native_callback_target_rows_for_tests as validate_native_callback_target_rows,
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
mod native_callback_arguments;
mod parameter_translation_fixture;
mod projected_result_qualifications;
mod ranked_countdown;
mod scalar;
mod structural_and_cleanup;
mod support;
mod translation_validation;
mod translation_validation_boolean;
mod translation_validation_boolean_equal_parameters;
mod translation_validation_boolean_not_parameter;
mod translation_validation_boolean_parameter;
mod translation_validation_byte_sequence_literal_unit_return;
mod translation_validation_crash;
mod translation_validation_exact_integer_add_parameters;
mod translation_validation_exact_integer_divide_parameters;
mod translation_validation_exact_integer_multiply_parameters;
mod translation_validation_exact_integer_remainder_parameters;
mod translation_validation_exact_integer_shift_left_parameters;
mod translation_validation_exact_integer_shift_right_parameters;
mod translation_validation_exact_integer_subtract_parameters;
mod translation_validation_ieee_float_literal_sequence_unit_return;
mod translation_validation_ieee_float_literal_unit_return;
mod translation_validation_integer_bitwise_and_parameters;
mod translation_validation_integer_bitwise_not_parameter;
mod translation_validation_integer_bitwise_or_parameters;
mod translation_validation_integer_bitwise_xor_parameters;
mod translation_validation_integer_equal_parameters;
mod translation_validation_integer_exact_cast_parameter;
mod translation_validation_integer_ieee_float_literal_sequence_unit_return;
mod translation_validation_integer_less_or_equal_parameters;
mod translation_validation_integer_less_than_parameters;
mod translation_validation_integer_literal_sequence_unit_return;
mod translation_validation_integer_literal_unit_return;
mod translation_validation_integer_parameter;
mod translation_validation_integer_widen_parameter;
mod translation_validation_nearest_ieee_float_fused_multiply_add_unit_return;
mod translation_validation_port_write_unit_return;
mod translation_validation_saturating_integer_add_parameters;
mod translation_validation_saturating_integer_divide_parameters;
mod translation_validation_saturating_integer_multiply_parameters;
mod translation_validation_saturating_integer_remainder_parameters;
mod translation_validation_saturating_integer_subtract_parameters;
mod translation_validation_trivial_affine_local_unit_return;
mod translation_validation_unit_call_return;
mod translation_validation_unit_return;
mod translation_validation_wrapping_integer_add_parameters;
mod translation_validation_wrapping_integer_divide_parameters;
mod translation_validation_wrapping_integer_multiply_parameters;
mod translation_validation_wrapping_integer_remainder_parameters;
mod translation_validation_wrapping_integer_shift_left_parameters;
mod translation_validation_wrapping_integer_shift_right_parameters;
mod translation_validation_wrapping_integer_subtract_parameters;
mod unit_and_settlements;
mod unit_scalar_calls;
mod unit_structural_calls;

pub(super) use support::{identity, scalar_result, scalar_result_mut};
