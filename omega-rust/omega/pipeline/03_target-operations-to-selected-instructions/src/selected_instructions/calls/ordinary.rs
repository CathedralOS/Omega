//! Semantic call and memory contracts attached to ordinary selected instructions.
use crate::selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId};
use terminal_psi_to_abstract_operations::optimization_unit::{EffectLink, OwnershipEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutgoingArgumentSlotId {
    pub operation: OperationId,
    pub argument_index: u32,
    pub role: OutgoingArgumentSlotRole,
}

/// An indirect argument's pointer and payload copy occupy distinct ABI regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutgoingArgumentSlotRole {
    /// Inline argument bytes or an indirectly passed argument's pointer.
    Argument,
    /// Caller-prepared payload at the retained indirect copy offset.
    ValueCopy,
}

impl OutgoingArgumentSlotId {
    /// Complete slot identity for versioned rosters without an enclosing frame tag.
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.operation.get().to_le_bytes());
        bytes.extend_from_slice(&self.argument_index.to_le_bytes());
        bytes.push(match self.role {
            OutgoingArgumentSlotRole::Argument => 0,
            OutgoingArgumentSlotRole::ValueCopy => 1,
        });
    }
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
        register: crate::selected_instructions::VirtualRegisterId,
    },
    Structural {
        operation: OperationId,
        place: PlaceId,
    },
    /// One transient descriptor per call operand, even when operands share backing.
    StructuralCallArgument {
        operation: OperationId,
        argument_index: u32,
    },
    Boundary {
        operation: OperationId,
    },
}

impl LocalStorageSlotId {
    pub const fn operation(self) -> Option<OperationId> {
        match self {
            Self::Structural { operation, .. }
            | Self::StructuralCallArgument { operation, .. }
            | Self::Boundary { operation } => Some(operation),
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
            Self::Boundary { .. } | Self::Spill { .. } | Self::StructuralCallArgument { .. } => {
                None
            }
        }
    }

    /// Tagged storage-origin identity; boundary scratch never fabricates a place.
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::StructuralCallArgument {
                operation,
                argument_index,
            } => {
                bytes.push(5);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
                bytes.extend_from_slice(&argument_index.to_le_bytes());
            }
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
    pub call: crate::legalized_operations::LegalizedScalarCall,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedMemoryAccessRole {
    /// Runtime span at byte_offset. byte_count must be zero; length is the
    /// authoritative extent, not a capacity or a fixed memory footprint.
    ReadByteSpan {
        length: semantic_vocabulary::ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Exact destination span of a non-overlapping copy; byte_count must be zero.
    WriteByteSpan {
        length: semantic_vocabulary::ValueId,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Exactly one written byte at the checked dynamic index, added to the row's
    /// fixed payload offset. The subject is a mutable view or bounded inline field.
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
    /// One runtime-selected element of a write through a verified structural
    /// projection (`grid[i][j] = v`, `ents[i].hp = v`). The written address is
    /// the subject's base plus the row's fixed `byte_offset` plus, for each
    /// runtime element of the path, its `index · stride`, scaled in path
    /// order by the instructions before the store. The access carries one
    /// such row per runtime element, in path order; each row's `extent` is
    /// its array's declared element count and `obligation`/`accepted_fact`
    /// the verifier's certificate that `index` lies inside it. Every index is
    /// non-negative, so the write lands at or after `byte_offset`; its exact
    /// position is a constant only when every row's index is.
    WriteIndexedPrimitive {
        index: semantic_vocabulary::ValueId,
        value: semantic_vocabulary::ValueId,
        stride: u32,
        extent: u64,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// One runtime-selected element of a read through a verified structural
    /// projection; the read counterpart of `WriteIndexedPrimitive`, with the
    /// same one-row-per-runtime-element address model.
    ReadIndexedPrimitive {
        index: semantic_vocabulary::ValueId,
        stride: u32,
        extent: u64,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// Exactly one checked element read at the dynamic element index. The
    /// element-to-byte scaling lives in the row's instruction sequence;
    /// `index` and `length` are element units.
    ReadElementView {
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
        layout:
            abstract_operations_to_target_operations::calling_conventions::ConventionalSumLayout,
    },
    HostedExitProcessI32 {
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        source: semantic_vocabulary::ValueId,
    },
    ClaimCompletion(crate::legalized_operations::LegalizedBoundarySettlement),
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
                crate::legalized_operations::encode_hosted_read_byte_identity(
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
