//! Optimizer module role: stage group.
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperation, TerminalPsiProvenance};
use psi_core::{
    BlockId, ClaimId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    Proposition, ScalarTerm, ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    CrashCause, CrashPredicateTerm, EntryClaim, SemanticFingerprint, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, TerminalPsiIdentity, VocabularyMarker,
};

use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineScalarCrashTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

use fixture::*;
