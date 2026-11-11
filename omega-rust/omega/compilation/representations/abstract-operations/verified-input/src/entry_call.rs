/*
  The `ProgramEntry` call that native admission selected for an artifact: the
  boundary the program's storage entry crosses, the checked entry receipt, the
  machine target selection resolved it to, and the provider plan that bound
  it. Omega 00 settles it (`bind_selected_program_entry_call`); realization
  stages read it from `VerifiedPsiOptimizationInput`.
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
    pub const fn settled(
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
