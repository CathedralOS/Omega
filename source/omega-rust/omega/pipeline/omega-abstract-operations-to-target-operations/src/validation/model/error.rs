use psi_core::MachineId;

use super::AbstractToTargetTranslationFamily;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationValidationError {
    PsiMismatch,
    TargetMismatch,
    EntryMismatch,
    FunctionCountMismatch,
    FunctionMachineMismatch {
        position: usize,
    },
    FunctionAttachmentMismatch {
        machine: MachineId,
    },
    AmbiguousFunctionFamily {
        machine: MachineId,
        first: AbstractToTargetTranslationFamily,
        second: AbstractToTargetTranslationFamily,
    },
    FunctionFamily {
        machine: MachineId,
        family: AbstractToTargetTranslationFamily,
        error: AbstractToTargetTranslationFamilyError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationFamilyError {
    StraightLineIntegerImmediate(StraightLineIntegerImmediateTranslationError),
    StraightLineBooleanImmediate(StraightLineBooleanImmediateTranslationError),
    StraightLineScalarCrash(StraightLineScalarCrashTranslationError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineBooleanImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineIntegerImmediateTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceConstantType,
    SourceConstantOutsideType,
    SourceResultLink,
    SourceCleanup,
    TargetProvenance,
    TargetOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineScalarCrashTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    TargetProvenance,
    TargetOperation,
}

impl std::fmt::Display for AbstractToTargetTranslationValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "abstract-to-target translation validation failed: {self:?}"
        )
    }
}

impl std::error::Error for AbstractToTargetTranslationValidationError {}
