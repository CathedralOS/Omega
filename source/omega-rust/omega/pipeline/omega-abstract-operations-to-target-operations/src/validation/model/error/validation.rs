//! Whole-translation validation failures above exact family replay.

use psi_core::{MachineId, OperationId};

use super::super::AbstractToTargetTranslationFamily;
use super::AbstractToTargetTranslationFamilyError;

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
    FunctionStructuralTypeRosterMismatch {
        machine: MachineId,
    },
    DuplicateIeeeFloatFmaSettlement(OperationId),
    UnknownIeeeFloatFmaSettlement(OperationId),
    MissingIeeeFloatFmaSettlement(OperationId),
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

impl std::fmt::Display for AbstractToTargetTranslationValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "abstract-to-target translation validation failed: {self:?}"
        )
    }
}

impl std::error::Error for AbstractToTargetTranslationValidationError {}
