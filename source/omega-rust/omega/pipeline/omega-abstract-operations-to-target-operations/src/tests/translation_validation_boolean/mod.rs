//! Optimizer module role: stage group.
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperation, TerminalPsiProvenance};
use psi_core::{
    BlockId, ClaimId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    EntryClaim, SemanticFingerprint, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
    VocabularyMarker,
};

use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineBooleanImmediateTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};

mod fixture;
mod positive;
mod source_corruption;
mod target_corruption;

#[path = "../translation_validation_boolean_equal_immediate/mod.rs"]
mod equal_immediate;
#[path = "../translation_validation_integer_equal_immediate/mod.rs"]
mod integer_equal_immediate;
#[path = "../translation_validation_boolean_not_immediate/mod.rs"]
mod not_immediate;

use fixture::*;
