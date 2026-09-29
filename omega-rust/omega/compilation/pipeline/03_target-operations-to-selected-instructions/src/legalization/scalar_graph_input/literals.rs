//! Exact local literal declarations and their earlier establishment occurrences.
use super::{AbstractOperation, PsiOptimizationFunction};
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};

pub(super) fn producer(
    function: &PsiOptimizationFunction,
    call: OperationId,
    source: PlaceId,
) -> Option<(OperationId, semantic_vocabulary::StructuralTypeId)> {
    function.blocks.iter().flat_map(|block| &block.nodes).find(|node| {
        matches!(&node.operation, AbstractOperation::CallStructural { psi_operation, structural_arguments, .. }
            | AbstractOperation::BoundaryCall { psi_operation, structural_arguments, .. }
            if *psi_operation == call && structural_arguments.iter().any(|argument| argument.place == source))
    })?;
    declaration_producer(function, source)
}

/// Rejoin one literal independently of unrelated storage or block arrangement.
/// Whole-unit validation checks ordinal density, producer dominance and borrow
/// availability. A declaration ordinal need not equal its place-roster position.
pub(super) fn declaration_producer(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Option<(OperationId, semantic_vocabulary::StructuralTypeId)> {
    let mut declarations = function
        .structural_places
        .iter()
        .filter(|place| place.id == source);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let StructuralPlaceKind::ByteSequenceLiteral {
        structural_type: identity,
        ..
    } = declaration.kind
    else {
        return None;
    };
    let mut producers = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                ..
            } if place.id == source => Some((*psi_operation, place, structural_type)),
            _ => None,
        });
    let (operation, place, structural_type) = producers.next()?;
    if producers.next().is_some()
        || place != declaration
        || structural_type.id != identity
        || !structural_type.shape.is_borrowed_byte_view()
    {
        return None;
    }
    Some((operation, identity))
}
