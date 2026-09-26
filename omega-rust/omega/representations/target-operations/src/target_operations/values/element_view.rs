//! Immutable element-view descriptors retain their source and checked derivation.
use crate::TargetIntegerExpression;
use crate::TargetStructuralArgumentSource;
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{BlockId, ObligationId, OperationId, PlaceId, StructuralTypeId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetElementView {
    /// Descriptor storage bound at this exact block entry, not a producer
    /// operation. The declaration distinguishes an owned aggregate home from
    /// a view descriptor.
    BlockParameter {
        block: BlockId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    /// The callee's incoming view descriptor placement.
    Parameter {
        place: PlaceId,
        placement: ValuePlacement,
    },
    /// A checked view establishment over a structural collection: the resolved
    /// projection supplies the backing storage and byte offset; the compile-
    /// known extent and element stride complete the descriptor.
    Established {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
        element: StructuralTypeId,
        root_structural_type: StructuralTypeId,
        source_byte_offset: u32,
        extent: u64,
        element_stride: u32,
        source: TargetStructuralArgumentSource,
    },
    /// A checked element-range derivation over a live descriptor. Start, end
    /// and the measured length retain proof custody; bounds were discharged
    /// at the obligation, not re-derived here.
    Subslice {
        psi_operation: OperationId,
        place: PlaceId,
        source: Box<TargetElementView>,
        start: Box<TargetIntegerExpression>,
        end: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: ObligationId,
    },
}
