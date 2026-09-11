//! Whole-plan and complete function-roster custody receipts.

use semantic_vocabulary::{MachineId, StructuralTypeId};
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
