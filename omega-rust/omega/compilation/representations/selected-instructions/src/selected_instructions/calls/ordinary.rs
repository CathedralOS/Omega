//! Semantic call and memory contracts attached to selected instructions.
//! Calls retain their real operation owner or exact cleanup edge/action owner.
use crate::{SelectedBlockId, SelectedInstructionId};
use optimization_unit::{EffectLink, OwnershipEvent};
use semantic_vocabulary::{BlockId, BoundaryInvocationSite, EdgeId, OperationId, PlaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutgoingArgumentSlotId {
    pub invocation_site: BoundaryInvocationSite,
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
        self.invocation_site.encode_identity(bytes);
        crate::codec_bytes::push_encoded(bytes, self.argument_index.to_le_bytes());
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
        register: crate::VirtualRegisterId,
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
    /// Private parallel-assignment staging, not a semantic place or ABI home.
    EdgeSnapshot {
        edge: semantic_vocabulary::EdgeId,
        binding_index: u32,
    },
    Boundary {
        operation: OperationId,
    },
    /// Caller-owned scalar result storage, disjoint from boundary controls.
    BoundaryResult {
        operation: OperationId,
    },
}

impl LocalStorageSlotId {
    pub const fn operation(self) -> Option<OperationId> {
        match self {
            Self::Structural { operation, .. }
            | Self::StructuralCallArgument { operation, .. }
            | Self::Boundary { operation }
            | Self::BoundaryResult { operation } => Some(operation),
            Self::Spill { .. }
            | Self::EdgeSnapshot { .. }
            | Self::StructuralBlockParameter { .. }
            | Self::StructuralParameter { .. } => None,
        }
    }

    pub const fn structural_place(self) -> Option<PlaceId> {
        match self {
            Self::Structural { place, .. }
            | Self::StructuralBlockParameter { place, .. }
            | Self::StructuralParameter { place } => Some(place),
            Self::Boundary { .. }
            | Self::BoundaryResult { .. }
            | Self::Spill { .. }
            | Self::StructuralCallArgument { .. }
            | Self::EdgeSnapshot { .. } => None,
        }
    }

    /// Tagged storage-origin identity; boundary scratch never fabricates a place.
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::EdgeSnapshot {
                edge,
                binding_index,
            } => {
                bytes.push(6);
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, binding_index.to_le_bytes());
            }
            Self::StructuralCallArgument {
                operation,
                argument_index,
            } => {
                bytes.push(5);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, argument_index.to_le_bytes());
            }
            Self::StructuralParameter { place } => {
                bytes.push(4);
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
            Self::StructuralBlockParameter { block, place } => {
                bytes.push(3);
                crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
            Self::Spill { register } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
            }
            Self::Structural { operation, place } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
            Self::BoundaryResult { operation } => {
                bytes.push(7);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            }
            Self::Boundary { operation } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
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

impl SelectedOutgoingArgumentSlot {
    /// Indirect copies own padding for absolute alignment within a fixed ABI
    /// frame. Direct argument positions remain exact ABI coordinates.
    pub fn reserved_byte_size(&self, stack_alignment: u16) -> Option<u32> {
        let padding = if self.id.role == OutgoingArgumentSlotRole::ValueCopy {
            u32::from(self.alignment.saturating_sub(stack_alignment))
        } else {
            0
        };
        self.byte_size.checked_add(padding)
    }

    pub fn reservation_alignment(&self, stack_alignment: u16) -> u16 {
        if self.id.role == OutgoingArgumentSlotRole::ValueCopy {
            self.alignment.min(stack_alignment)
        } else {
            self.alignment
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCallContract {
    pub instruction: SelectedInstructionId,
    pub owner: target_operations::CallSiteOwner,
    pub call: legalized_operations::LegalizedScalarCall,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedMemoryAccessRole {
    /// Read through a completed descriptor or an aggregate's reference leaf.
    /// `producer` names the exact call whose retained roster owns the backing,
    /// including literal machine namespaces. The row's place is the result, not an
    /// independent allocation; offset/count are zero and grant no disjointness.
    /// Until origin-aware alias analysis supplies a narrower proof, this read
    /// interferes with every store. Descriptor payload loads remain ReadPlace.
    ReadReturnedView {
        producer: OperationId,
    },
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
    /// Runtime span at byte_offset; byte_count must be zero. `length` is the
    /// operand's own dominating length measurement, so the extent is exactly
    /// what the operand spells: the construction binds the read authority and
    /// the access names no obligation certificate.
    ReadMeasuredSpan {
        length: semantic_vocabulary::ValueId,
    },
    /// One runtime-selected element of a write through a verified structural
    /// projection (`grid[i][j] = v`, `ents[i].hp = v`). The written address is
    /// the subject's base plus the row's fixed `byte_offset` plus, for each
    /// runtime element of the path, its `index · stride`, scaled in path
    /// order by the instructions before the store. The access carries one
    /// such row per runtime element, in path order; each row's `extent` is
    /// its array's declared element count — `None` when the carrier is an
    /// element view, whose extent is the live length observation the
    /// certificate names — and `obligation`/`accepted_fact` the verifier's
    /// certificate that `index` lies inside it. Every index is
    /// non-negative, so the write lands at or after `byte_offset`; its exact
    /// position is a constant only when every row's index is.
    WriteIndexedPrimitive {
        index: semantic_vocabulary::ValueId,
        value: semantic_vocabulary::ValueId,
        stride: u32,
        extent: Option<u64>,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// One runtime-selected element of a write through a verified structural
    /// projection — the write counterpart of `ReadIndexedPrimitive` for a
    /// byte copy (a borrowed window's reseat), which stores chunks of a
    /// structural element and so names no single scalar `value`. Same
    /// one-row-per-runtime-element address model and certificate shape as
    /// `WriteIndexedPrimitive`.
    WriteIndexedStructural {
        index: semantic_vocabulary::ValueId,
        stride: u32,
        extent: Option<u64>,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
    /// One runtime-selected element of a read through a verified structural
    /// projection; the read counterpart of `WriteIndexedPrimitive`, with the
    /// same one-row-per-runtime-element address model.
    ReadIndexedPrimitive {
        index: semantic_vocabulary::ValueId,
        stride: u32,
        extent: Option<u64>,
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
    /// Exactly one checked element store at the dynamic element index through
    /// the view's exclusive loan — the write counterpart of `ReadElementView`.
    WriteElementView {
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
        crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
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
        edge: semantic_vocabulary::EdgeId,
        execution: target_operations::BoundaryExecutionBinding,
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
    pub fn invocation_site(&self) -> semantic_vocabulary::BoundaryInvocationSite {
        match self {
            Self::ClaimCompletion(settlement) => settlement.operation.into(),
            Self::HostedReadByte { operation, .. } | Self::HostedWriteByteI32 { operation, .. } => {
                (*operation).into()
            }
            Self::HostedExitProcessI32 { edge, .. } => {
                semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(*edge)
            }
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
                edge,
                execution,
                boundary,
                source,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(&mut bytes, edge.get().to_le_bytes());
                legalized_operations::identity::encode_boundary_execution_identity(
                    &mut bytes, *execution,
                );
                crate::codec_bytes::push_encoded(&mut bytes, boundary.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, source.get().to_le_bytes());
            }
            Self::HostedWriteByteI32 {
                operation,
                boundary,
                source,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(&mut bytes, operation.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, boundary.get().to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, source.get().to_le_bytes());
            }
        }
        bytes
    }
}
