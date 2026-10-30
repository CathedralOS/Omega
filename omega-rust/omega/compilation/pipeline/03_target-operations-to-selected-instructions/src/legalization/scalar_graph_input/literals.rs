//! Exact local literal declarations and their earlier establishment occurrences.
use super::{AbstractOperation, PsiOptimizationFunction};
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};

pub(super) fn producer(
    function: &PsiOptimizationFunction,
    call: semantic_vocabulary::InvocationSite,
    source: PlaceId,
) -> Option<(OperationId, semantic_vocabulary::StructuralTypeId)> {
    super::invocations::position(function, call, None)?;
    function.block_nodes().find(|node| {
        super::invocations::matches(&node.operation, call)
            && node.operation.module_invocation().is_some_and(|invocation|
                invocation.structural_arguments.iter().any(|argument| argument.place == source))
            || matches!(&node.operation,
                abstract_operations::boundary_or_structural_call_pair_pattern!(psi_operation, structural_arguments, ..)
                if call == semantic_vocabulary::InvocationSite::Operation(*psi_operation)
                    && structural_arguments.iter().any(|argument| argument.place == source))
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
        .block_nodes()
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
