//! Immutable view descriptors retain their source and checked derivation.
use crate::TargetIntegerExpression;
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{ObligationId, OperationId, PlaceId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetByteView {
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
