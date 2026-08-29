//! Whole-plan and complete function-roster custody receipts.

use omega_target::NativeTarget;
use psi_core::{MachineId, StructuralTypeId};
use psi_terminal::TerminalPsiIdentity;

use super::AbstractToTargetFunctionTranslationReceipt;

/// Independent source-to-target validation retained at the lowering boundary.
///
/// Root and function-roster custody cover the complete plan. Every function
/// row carries exactly one validated family receipt or an explicit uncovered
/// disposition, so parallel family rosters cannot drift apart.
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
    translation: AbstractToTargetFunctionTranslationDisposition,
}

impl AbstractToTargetFunctionRosterReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        attachment: Option<StructuralTypeId>,
        translation: AbstractToTargetFunctionTranslationDisposition,
    ) -> Self {
        Self {
            machine,
            attachment,
            translation,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn attachment(&self) -> Option<StructuralTypeId> {
        self.attachment
    }

    pub const fn translation(&self) -> &AbstractToTargetFunctionTranslationDisposition {
        &self.translation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetFunctionTranslationDisposition {
    Uncovered,
    Validated(AbstractToTargetFunctionTranslationReceipt),
}

impl AbstractToTargetFunctionTranslationDisposition {
    pub const fn validated(&self) -> Option<&AbstractToTargetFunctionTranslationReceipt> {
        match self {
            Self::Uncovered => None,
            Self::Validated(receipt) => Some(receipt),
        }
    }
}
