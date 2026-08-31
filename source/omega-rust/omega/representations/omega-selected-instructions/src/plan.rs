use omega_target::NativeTarget;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{SelectedFunction, SelectedStructuralUnitFunction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstructionPlan {
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<SelectedFunction>,
    /// Structural-ABI Unit functions are deliberately kept out of the scalar
    /// VReg roster. Their selected call bundle has no allocator-managed value
    /// and cannot acquire a fabricated scalar operand merely to enter the
    /// ordinary instruction vocabulary.
    pub structural_unit_functions: Vec<SelectedStructuralUnitFunction>,
}
