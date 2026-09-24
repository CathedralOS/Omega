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
mod reference_results;
mod structural_argument_sources;
mod structural_call_arguments;
mod structural_shapes;
mod structural_signatures;
mod whole_plan;

pub use whole_plan::{
    validate_abstract_to_target_translation,
    validate_abstract_to_target_translation_with_ieee_float_fma_settlements,
};

use semantic_vocabulary::{MachineId, OperationId, StructuralTypeId};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

/// Independent source-to-target validation retained at the lowering boundary.
///
/// The receipt covers roots, declarations, and ABI headers, not executable bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractToTargetTranslationValidationReceipt {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    function_roster: Vec<AbstractToTargetFunctionRosterReceipt>,
}

impl AbstractToTargetTranslationValidationReceipt {
    pub(in crate::validation) fn new(
        psi: TerminalPsiIdentity,
        target: NativeTarget,
        entry: MachineId,
        function_roster: Vec<AbstractToTargetFunctionRosterReceipt>,
    ) -> Self {
        Self {
            psi,
            target,
            entry,
            function_roster,
        }
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn function_count(&self) -> usize {
        self.function_roster.len()
    }

    pub fn function_roster(&self) -> &[AbstractToTargetFunctionRosterReceipt] {
        &self.function_roster
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractToTargetFunctionRosterReceipt {
    machine: MachineId,
    attachment: Option<StructuralTypeId>,
}

impl AbstractToTargetFunctionRosterReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        attachment: Option<StructuralTypeId>,
    ) -> Self {
        Self {
            machine,
            attachment,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn attachment(&self) -> Option<StructuralTypeId> {
        self.attachment
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationValidationError {
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
    DuplicateIeeeFloatFmaSettlement(OperationId),
    UnknownIeeeFloatFmaSettlement(OperationId),
    MissingIeeeFloatFmaSettlement(OperationId),
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
