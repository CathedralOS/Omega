//! Function-relative bytes awaiting final text placement and fixup resolution.

pub mod control_flow;
pub mod fixups;
pub mod functions;
pub mod identity;
pub mod publication;

pub use control_flow::*;
pub use fixups::*;
pub use functions::*;
pub use identity::function_fragment_emission_identity;
pub use publication::*;

use omega_optimization_core::FunctionFragmentEmissionIdentity;
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use omega_target::NativeTarget;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionPlan {
    pub identity: FunctionFragmentEmissionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<FunctionFragment>,
    pub structural_unit_functions: Vec<StructuralUnitFunctionFragment>,
}

impl FunctionFragmentEmissionPlan {
    pub fn recomputed_identity(&self) -> FunctionFragmentEmissionIdentity {
        function_fragment_emission_identity(self)
    }
}

#[cfg(test)]
mod tests;
