//! control flow functions in the assigned operations program.

use crate::AssignedOperation;
use semantic_vocabulary::MachineId;
use semantic_vocabulary::StructuralTypeId;
use target_operations::TerminalPsiProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub fixed_integer_scalar_abi: Option<target_operations::FixedIntegerScalarFunctionAbi>,
    pub mixed_structural_scalar_abi: Option<target_operations::MixedStructuralScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub operation: AssignedOperation,
}
