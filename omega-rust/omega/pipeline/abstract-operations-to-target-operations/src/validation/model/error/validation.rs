//! Whole-translation validation failures above exact family replay.

use semantic_vocabulary::{MachineId, OperationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationValidationError {
    StructuralSignatureMismatch {
        machine: MachineId,
    },
    UnsupportedPartialAffineContinuation {
        machine: MachineId,
        edge: semantic_vocabulary::EdgeId,
    },
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
