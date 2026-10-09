//! Function-relative bytes awaiting final text placement and fixup resolution.

pub mod control_flow;
pub mod fixups;
pub mod functions;
pub mod identity;
pub mod publication;

pub(crate) use control_flow::FunctionFragmentJumpEvidence;
pub use control_flow::{
    FunctionFragmentBranchEvidence, FunctionFragmentConditionalBranchEvidence,
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentSuccessorProvenance,
};
pub use fixups::{
    FunctionFragmentCallbackMaterialization, FunctionFragmentCallbackMaterializationEncoding,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentNormalizedForeignCallFixup,
    FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState,
};
pub use functions::{FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentInstructionSpan};
pub(crate) use identity::function_fragment_emission_identity;
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
    /// The fragment for `machine`, when the plan carries it.
    pub fn function(&self, machine: semantic_vocabulary::MachineId) -> Option<&FunctionFragment> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    /// Every fragment's block spans, in function order.
    pub fn blocks(&self) -> impl Iterator<Item = &FunctionFragmentBlockSpan> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    /// Every fragment's instruction spans, in function then block order.
    pub fn instructions(&self) -> impl Iterator<Item = &FunctionFragmentInstructionSpan> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.instructions())
    }

    pub fn recomputed_identity(&self) -> FunctionFragmentEmissionIdentity {
        function_fragment_emission_identity(self)
    }
}

#[cfg(test)]
mod tests;
