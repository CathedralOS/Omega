//! Optimizer module role: stage group. Trivial affine-local Unit cleanup custody by behavior.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi, TargetOperation,
    TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use terminal_psi::{
    ByteSequenceCarrier, EntryClaim, SemanticFingerprint, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};

use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineTrivialAffineLocalUnitReturnTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

use fixture::*;
