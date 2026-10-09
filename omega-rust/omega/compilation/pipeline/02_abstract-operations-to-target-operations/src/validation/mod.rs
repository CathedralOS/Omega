//! Optimizer module role: stage group.
//! Independent root, function-roster, signature, and occurrence settlement
//! custody. Executable graph bodies require the downstream common graph
//! replay.
//!
//! `whole_plan` is the entry: it replays the plan identity, the evidence
//! rosters and the function roster, descending into `structural_signatures`,
//! `structural_shapes`, `structural_call_arguments`,
//! `structural_argument_sources`, `reference_results` and `installed_calls`
//! for each family. What validation produces on success -- the translation
//! and function-roster receipts -- and how it fails are defined here, beside
//! the families that mint and raise them.

pub(crate) mod installed_calls;
mod structural_call_arguments;
pub(crate) mod structural_shapes;
pub(crate) mod whole_plan;

use semantic_vocabulary::{MachineId, OperationId};





#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationValidationError {
    UnsupportedIeeeFloatFmaRealization(OperationId),
    StructuralSignatureMismatch {
        machine: MachineId,
    },
    StructuralCallArgumentMismatch {
        machine: MachineId,
        operation: OperationId,
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
    NativeCallbackRosterMismatch(OperationId),
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
