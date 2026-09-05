//! Plain allocation-to-machine evidence; construction grants no machine admission.
use crate::PostAllocationMachineIdentity;
use register_homes::AllocationEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineCustodyReceipt {
    pub source: AllocationEvidence,
    pub effects: selected_instructions::PreAllocationMachineEffectIdentity,
    pub machine: PostAllocationMachineIdentity,
    pub function_count: usize,
    pub structural_unit_function_count: usize,
    pub instruction_count: usize,
    pub operand_count: usize,
    pub unit_action_count: usize,
}

impl PostAllocationMachineCustodyReceipt {
    pub const fn source(&self) -> &AllocationEvidence {
        &self.source
    }
    pub const fn effects(&self) -> selected_instructions::PreAllocationMachineEffectIdentity {
        self.effects
    }
    pub const fn machine(&self) -> PostAllocationMachineIdentity {
        self.machine
    }
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(&self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count
    }
    pub const fn operand_count(&self) -> usize {
        self.operand_count
    }
    pub const fn unit_action_count(&self) -> usize {
        self.unit_action_count
    }
}
