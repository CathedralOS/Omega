//! Immutable view descriptors retain their source and checked derivation.
use crate::calling_conventions::ValuePlacement;
use crate::target_operations::TargetIntegerExpression;
use semantic_vocabulary::{BlockId, ObligationId, OperationId, PlaceId, StructuralTypeId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetByteView {
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
}
