//! Optimizer module role: stage group. Parameterless Unit-call translation custody by behavior.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, IntegerSign, IntegerType, MachineId, ObligationId, OperationId,
    PlaceId, ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi, TargetOperation,
    TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation,
    TerminalPsiProvenance,
};
use terminal_psi::{
    ClaimTransfer, CrashCause, CrashRouteBucket, CrashRouteGuard, EntryClaim, SemanticFingerprint,
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};

use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineUnitCallReturnTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

use fixture::*;
