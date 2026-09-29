//! Immutable view descriptors retain their source and checked derivation.
use crate::TargetIntegerExpression;
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{BlockId, ObligationId, OperationId, PlaceId, StructuralTypeId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetByteView {
    /// Immutable read descriptor for fresh bounded storage in this activation.
    Owned {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    /// Immutable backing established by this exact operation, not a parameter
    /// descriptor or authority to manufacture additional literal bytes.
    Literal {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    BlockParameter {
        block: BlockId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Parameter {
        place: PlaceId,
        placement: ValuePlacement,
    },
    Subslice {
        psi_operation: OperationId,
        place: PlaceId,
        source: Box<TargetByteView>,
        start: Box<TargetIntegerExpression>,
        end: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: ObligationId,
    },
    /// A borrowed descriptor returned by a call in this activation. The
    /// call's result home already holds the (pointer, length) pair; the view
    /// names the producing operation so consumers load the descriptor fields
    /// the way they read a parameter descriptor.
    StructuralResult {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
}
