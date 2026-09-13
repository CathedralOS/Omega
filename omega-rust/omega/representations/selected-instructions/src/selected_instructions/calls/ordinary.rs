//! Semantic call and memory contracts attached to ordinary selected instructions.
use crate::{SelectedBlockId, SelectedInstructionId};
use optimization_unit::{EffectLink, OwnershipEvent};
use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutgoingArgumentSlotId {
    pub operation: OperationId,
    pub argument_index: u32,
}

/// Activation-local storage identity, independent of any call's ABI copies.
/// Owned input storage retains value bytes; it does not copy a borrowed referent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LocalStorageSlotId {
    /// Function-owned input storage identified by its exact parameter place.
    StructuralParameter {
        place: PlaceId,
    },
    StructuralBlockParameter {
        block: BlockId,
        place: PlaceId,
    },
    /// One compiler-owned slot per original virtual value, scoped to its function.
    Spill {
        register: crate::VirtualRegisterId,
    },
    Structural {
        operation: OperationId,
        place: PlaceId,
    },
    Boundary {
        operation: OperationId,
    },
}

impl LocalStorageSlotId {
    pub const fn operation(self) -> Option<OperationId> {
        match self {
            Self::Structural { operation, .. } | Self::Boundary { operation } => Some(operation),
            Self::Spill { .. }
            | Self::StructuralBlockParameter { .. }
            | Self::StructuralParameter { .. } => None,
        }
    }

    pub const fn structural_place(self) -> Option<PlaceId> {
        match self {
            Self::Structural { place, .. }
            | Self::StructuralBlockParameter { place, .. }
            | Self::StructuralParameter { place } => Some(place),
            Self::Boundary { .. } | Self::Spill { .. } => None,
        }
    }

    /// Tagged storage-origin identity; boundary scratch never fabricates a place.
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::StructuralParameter { place } => {
                bytes.push(4);
                bytes.extend_from_slice(&place.get().to_le_bytes());
            }
            Self::StructuralBlockParameter { block, place } => {
                bytes.push(3);
                bytes.extend_from_slice(&block.get().to_le_bytes());
                bytes.extend_from_slice(&place.get().to_le_bytes());
            }
            Self::Spill { register } => {
                bytes.push(2);
                bytes.extend_from_slice(&register.0.to_le_bytes());
            }
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
    /// Incoming ABI slot, outside the callee-owned frame; offsets exclude its prologue and return address.
    Incoming {
        parameter_index: u32,
        abi_stack_byte_offset: u32,
    },
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
    /// Exactly one written byte at the checked dynamic index in the current mutable view.
    WriteByteSequence {
        index: semantic_vocabulary::ValueId,
        value: semantic_vocabulary::ValueId,
        length: semantic_vocabulary::ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Dynamic byte offset; the row's fixed byte offset is additive only.
    ReadByteSequence {
        index: semantic_vocabulary::ValueId,
        length: semantic_vocabulary::ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    ReadPlace,
    WritePlace,
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
    pub origin: SelectedMemoryAccessOrigin,
    pub place: PlaceId,
    pub byte_offset: u32,
    pub byte_count: u32,
    pub role: SelectedMemoryAccessRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedMemoryAccessOrigin {
    Operation(OperationId),
    Edge(EdgeId),
    Block(BlockId),
}

impl SelectedMemoryAccessOrigin {
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        let (tag, identity) = match self {
            Self::Operation(identity) => (0, identity.get()),
            Self::Edge(identity) => (1, identity.get()),
            Self::Block(identity) => (2, identity.get()),
        };
        bytes.push(tag);
        bytes.extend_from_slice(&identity.to_le_bytes());
    }
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
    HostedReadByte {
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        result: terminal_psi::StructuralOperationResult,
        layout: calling_conventions::ConventionalSumLayout,
    },
    HostedExitProcessI32 {
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        source: semantic_vocabulary::ValueId,
    },
    ClaimCompletion(legalized_operations::LegalizedBoundarySettlement),
    HostedWriteByteI32 {
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        source: semantic_vocabulary::ValueId,
    },
}

impl SelectedBoundarySettlementPayload {
    pub fn operation(&self) -> OperationId {
        match self {
            Self::ClaimCompletion(settlement) => settlement.operation,
            Self::HostedReadByte { operation, .. }
            | Self::HostedWriteByteI32 { operation, .. }
            | Self::HostedExitProcessI32 { operation, .. } => *operation,
        }
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            Self::HostedReadByte {
                operation,
                boundary,
                result,
                layout,
            } => {
                bytes.push(3);
                legalized_operations::encode_hosted_read_byte_identity(
                    &mut bytes, *operation, *boundary, result, layout,
                );
            }
            Self::ClaimCompletion(settlement) => {
                bytes.push(0);
                bytes.extend_from_slice(&settlement.canonical_bytes());
            }
            Self::HostedExitProcessI32 {
                operation,
                boundary,
                source,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
                bytes.extend_from_slice(&boundary.get().to_le_bytes());
                bytes.extend_from_slice(&source.get().to_le_bytes());
            }
            Self::HostedWriteByteI32 {
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
