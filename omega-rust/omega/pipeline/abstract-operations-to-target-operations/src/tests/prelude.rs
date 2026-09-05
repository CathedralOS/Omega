//! Shared test vocabulary for abstract-to-target lowering fixtures.

pub(super) use crate::lowering::{
    bind_native_callback_arguments_for_tests as bind_native_callback_arguments,
    lower_with_settlements_for_tests as lower_to_target_operations_with_settlements,
    structural_shape_for_tests as structural_shape,
    validate_native_callback_target_rows_for_tests as validate_native_callback_target_rows,
};
pub(super) use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractSuccessor, ValueBinding,
};
pub(super) use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
pub(super) use installation_evidence::{
    InstalledProviderCompletionClaimSource, InstalledProviderUnitCallEvidence,
    ProviderInstallationEvidence,
};
pub(super) use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
pub(super) use std::collections::{BTreeMap, BTreeSet};
pub(super) use target::NativeTarget;
pub(super) use target_operations::{
    BoundaryRealization, BoundarySettlementBinding, MachineRegister, ScalarParameterLocation,
    TargetBooleanControl, TargetBooleanExpression, TargetIntegerExpression, TargetOperation,
    TargetUnitOperation, TargetUnitScalarArgumentSource,
};
pub(super) use terminal_psi::{
    BoundaryMachineDeclaration, CrashCause, CrashRouteBucket, CrashRouteGuard, SemanticFingerprint,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    TerminalPsiIdentity, VocabularyMarker,
};
