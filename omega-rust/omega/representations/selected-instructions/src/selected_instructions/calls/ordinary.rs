//! Semantic call and memory contracts attached to ordinary selected instructions.
use crate::{SelectedBlockId, SelectedInstructionId};
use optimization_unit::{EffectLink, OwnershipEvent};
use semantic_vocabulary::{OperationId, PlaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutgoingArgumentSlotId {
    pub operation: OperationId,
    pub argument_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedOutgoingArgumentSlot {
    pub id: OutgoingArgumentSlotId,
    pub byte_size: u32,
    pub alignment: u16,
    /// Caller-copy offset in the callee ABI, not a resolved stack address.
    pub abi_stack_byte_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCallContract {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub call: legalized_operations::LegalizedScalarCall,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedMemoryAccessRole {
    ReadPlace,
    WriteOutgoing { slot: OutgoingArgumentSlotId },
    AddressOutgoing { slot: OutgoingArgumentSlotId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedMemoryAccess {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub place: PlaceId,
    pub byte_offset: u32,
    pub byte_count: u32,
    pub role: SelectedMemoryAccessRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedBoundarySettlement {
    pub block: SelectedBlockId,
    /// Position before this instruction ordinal (or after the block body).
    pub instruction_index: u32,
    pub settlement: legalized_operations::LegalizedBoundarySettlement,
}
