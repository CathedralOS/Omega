//! Semantic call and memory contracts attached to ordinary selected instructions.
use crate::{SelectedBlockId, SelectedInstructionId};
use optimization_unit::{EffectLink, OwnershipEvent};
use semantic_vocabulary::{OperationId, PlaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutgoingArgumentSlotId {
    pub operation: OperationId,
    pub argument_index: u32,
}

/// Activation-local storage identity, independent of any call's ABI copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LocalStorageSlotId {
    Structural {
        operation: OperationId,
        place: PlaceId,
    },
    Boundary {
        operation: OperationId,
    },
}

impl LocalStorageSlotId {
    pub const fn operation(self) -> OperationId {
        match self {
            Self::Structural { operation, .. } | Self::Boundary { operation } => operation,
        }
    }

    pub const fn structural_place(self) -> Option<PlaceId> {
        match self {
            Self::Structural { place, .. } => Some(place),
            Self::Boundary { .. } => None,
        }
    }

    /// Tagged storage-origin identity; boundary scratch never fabricates a place.
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::Structural { operation, place } => {
                bytes.push(0);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
                bytes.extend_from_slice(&place.get().to_le_bytes());
            }
            Self::Boundary { operation } => {
                bytes.push(1);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FrameStorageSlotId {
    Outgoing(OutgoingArgumentSlotId),
    Local(LocalStorageSlotId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedLocalStorageSlot {
    pub id: LocalStorageSlotId,
    pub byte_size: u32,
    pub alignment: u16,
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
    /// Dynamic byte offset; the row's fixed byte offset is additive only.
    ReadByteSequence {
        index: semantic_vocabulary::ValueId,
        length: semantic_vocabulary::ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    ReadPlace,
    WriteLocal {
        slot: LocalStorageSlotId,
    },
    AddressLocal {
        slot: LocalStorageSlotId,
    },
    WriteOutgoing {
        slot: OutgoingArgumentSlotId,
    },
    AddressOutgoing {
        slot: OutgoingArgumentSlotId,
    },
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
    pub settlement: SelectedBoundarySettlementPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedBoundarySettlementPayload {
    ClaimCompletion(legalized_operations::LegalizedBoundarySettlement),
    LinuxWriteByteI32 {
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        source: semantic_vocabulary::ValueId,
    },
}

impl SelectedBoundarySettlementPayload {
    pub fn operation(&self) -> OperationId {
        match self {
            Self::ClaimCompletion(settlement) => settlement.operation,
            Self::LinuxWriteByteI32 { operation, .. } => *operation,
        }
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            Self::ClaimCompletion(settlement) => {
                bytes.push(0);
                bytes.extend_from_slice(&settlement.canonical_bytes());
            }
            Self::LinuxWriteByteI32 {
                operation,
                boundary,
                source,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
                bytes.extend_from_slice(&boundary.get().to_le_bytes());
                bytes.extend_from_slice(&source.get().to_le_bytes());
            }
        }
        bytes
    }
}
