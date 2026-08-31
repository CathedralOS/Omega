use omega_target::NativeTarget;
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{MachineId, StructuralTypeId};
use psi_terminal::TerminalPsiIdentity;

use crate::AssignedOperation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<AssignedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub fixed_integer_scalar_abi: Option<omega_target_operations::FixedIntegerScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub operation: AssignedOperation,
}
