// We define machine functions and basic blocks in the post-allocation physical
// program, together with their frame-local storage slots and outgoing call
// argument slots.

use crate::PostAllocationMachineInstruction;
use selected_instructions::{SelectedBlockId, SelectedOutgoingArgumentSlot};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineFunction {
    pub machine: MachineId,
    pub local_storage_slots: Vec<selected_instructions::SelectedLocalStorageSlot>,
    pub outgoing_arguments: Vec<SelectedOutgoingArgumentSlot>,
    pub blocks: Vec<PostAllocationMachineBlock>,
}

impl PostAllocationMachineFunction {
    pub fn instructions(&self) -> impl Iterator<Item = &PostAllocationMachineInstruction> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineBlock {
    pub block: SelectedBlockId,
    pub instructions: Vec<PostAllocationMachineInstruction>, // Ordinary selected instructions followed by the selected terminator.
}
