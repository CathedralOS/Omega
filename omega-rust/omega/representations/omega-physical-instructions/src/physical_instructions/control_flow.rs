//! Ordinary and structural functions in the physical program.

use crate::PostAllocationMachineInstruction;
use omega_selected_instructions::{SelectedBlockId, StructuralUnitCallMachineEffects};
use psi_core::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineFunction {
    pub machine: MachineId,
    pub blocks: Vec<PostAllocationMachineBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineBlock {
    pub block: SelectedBlockId,
    /// Ordinary selected instructions followed by the selected terminator.
    pub instructions: Vec<PostAllocationMachineInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationStructuralUnitFunction {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub call: Option<StructuralUnitCallMachineEffects>,
    pub return_instruction: PostAllocationMachineInstruction,
    pub return_provenance: omega_selected_instructions::SelectedInstructionProvenance,
    pub return_effect: omega_optimization_unit::EffectLink,
    pub return_ownership: Vec<omega_optimization_unit::OwnershipEvent>,
}
