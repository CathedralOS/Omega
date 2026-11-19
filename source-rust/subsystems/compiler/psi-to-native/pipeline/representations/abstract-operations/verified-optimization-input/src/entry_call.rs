/*
  Keep the program's entry call tied to the exact artifact and provider that
  established it. Checked entry settlement records the crossed boundary, target
  machine and provider plan; realization reads them from the verified input.
*/
use semantic_vocabulary::{BoundaryMachineId, MachineId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntryCall {
    boundary: BoundaryMachineId,
    entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    callee: MachineId,
    provider_plan: effects::provider_plan::ProviderPlanDigest,
}

impl SelectedProgramEntryCall {
    pub(crate) const fn settled(
        boundary: BoundaryMachineId,
        entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
        callee: MachineId,
        provider_plan: effects::provider_plan::ProviderPlanDigest,
    ) -> Self {
        Self {
            boundary,
            entry,
            callee,
            provider_plan,
        }
    }
    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }
    pub const fn entry(&self) -> MachineId {
        self.callee
    }
    pub const fn source_signature(&self) -> [u8; 32] {
        self.entry.source_signature_identity()
    }
    pub const fn provider_plan(&self) -> effects::provider_plan::ProviderPlanDigest {
        self.provider_plan
    }
}
