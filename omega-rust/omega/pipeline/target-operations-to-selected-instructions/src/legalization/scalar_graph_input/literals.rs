//! Exact local literal declarations and their earlier establishment occurrences.
use super::*;
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};

pub(super) fn roster(function: &PsiOptimizationFunction) -> bool {
    if function.structural_places.is_empty() {
        return function.declared_places.is_empty()
            && !function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::EstablishByteSequenceLiteral { .. }
                    )
                });
    }
    let [block] = function.blocks.as_slice() else {
        return false;
    };
    if function.declared_places
        != function
            .structural_places
            .iter()
            .map(|place| place.id)
            .collect()
    {
        return false;
    }
    function.structural_places.iter().enumerate().all(|(ordinal, declaration)| {
        let Some(node) = block.nodes.get(ordinal) else { return false; };
        matches!(&node.operation, AbstractOperation::EstablishByteSequenceLiteral { place, structural_type, .. }
            if place == declaration
                && place.kind == StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: ordinal as u32, structural_type: structural_type.id,
                }
                && structural_type.shape == terminal_psi::StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView))
    }) && block.nodes.iter().filter(|node| matches!(node.operation, AbstractOperation::EstablishByteSequenceLiteral { .. })).count() == function.structural_places.len()
}

pub(super) fn producer(
    function: &PsiOptimizationFunction,
    call: OperationId,
    source: PlaceId,
) -> Option<(OperationId, semantic_vocabulary::StructuralTypeId)> {
    let [block] = function.blocks.as_slice() else {
        return None;
    };
    let call_position = block.nodes.iter().position(|node| {
        matches!(&node.operation, AbstractOperation::CallStructuralScalar { psi_operation, .. }
            | AbstractOperation::CallUnit { psi_operation, .. } if *psi_operation == call)
    })?;
    block.nodes[..call_position]
        .iter()
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                ..
            } if place.id == source => Some((*psi_operation, structural_type.id)),
            _ => None,
        })
}
