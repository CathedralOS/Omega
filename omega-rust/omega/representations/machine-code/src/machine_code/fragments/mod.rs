//! Function-relative bytes awaiting final text placement and fixup resolution.

pub mod control_flow;
pub mod fixups;
pub mod functions;
pub mod identity;
pub mod publication;

pub use control_flow::{
    FunctionFragmentBranchEvidence, FunctionFragmentConditionalBranchEvidence,
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentJumpEvidence, FunctionFragmentSuccessorProvenance,
};
pub use fixups::{
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentNormalizedForeignCallFixup,
    FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState,
};
pub use functions::{FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentInstructionSpan};
pub use identity::function_fragment_emission_identity;
pub use publication::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionManifestDecodeError,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData,
};

use optimization_core::FunctionFragmentEmissionIdentity;
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionPlan {
    pub identity: FunctionFragmentEmissionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<FunctionFragment>,
}

impl FunctionFragmentEmissionPlan {
    pub fn recomputed_identity(&self) -> FunctionFragmentEmissionIdentity {
        function_fragment_emission_identity(self)
    }
}

#[cfg(test)]
mod tests;
