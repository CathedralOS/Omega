//! Ordinary and structural functions in the physical program.

use crate::PostAllocationMachineInstruction;
use selected_instructions::{SelectedBlockId, SelectedOutgoingArgumentSlot};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineFunction {
    pub machine: MachineId,
    pub outgoing_arguments: Vec<SelectedOutgoingArgumentSlot>,
    pub blocks: Vec<PostAllocationMachineBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineBlock {
    pub block: SelectedBlockId,
    /// Ordinary selected instructions followed by the selected terminator.
    pub instructions: Vec<PostAllocationMachineInstruction>,
}
