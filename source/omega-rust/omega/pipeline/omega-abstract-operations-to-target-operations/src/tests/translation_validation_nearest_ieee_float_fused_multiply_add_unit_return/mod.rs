//! Optimizer module role: stage group. Exact nearest-even IEEE FMA Unit custody.

use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use omega_target::{
    NativeTarget, TargetProfile, X86_SCALAR_FMA_REQUIRED_FEATURES, X86ScalarFmaSlot,
};
use omega_target_operations::{
    FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi, TargetOperation,
    TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use psi_core::{
    BlockId, ClaimId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, MachineId,
    OperationId, PlaceId, ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    EntryClaim, SemanticFingerprint, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};

use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, AdmittedIeeeFloatFmaSettlement,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError,
    lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma,
    validate_abstract_to_target_translation_with_ieee_float_fma_settlements,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

use fixture::*;
