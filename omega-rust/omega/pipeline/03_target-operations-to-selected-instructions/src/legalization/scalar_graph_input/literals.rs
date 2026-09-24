//! Exact local literal declarations and their earlier establishment occurrences.
use super::{AbstractOperation, PsiOptimizationFunction};
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};

/// Provider roots are specialization witnesses, not literal storage or ABI inputs.
/// The enclosing unit custody check independently validates their exact field,
/// boundary roster, and service authority. Keep this separate from `roster` so
/// metadata alone never manufactures a runtime structural contract.
pub(super) fn provider_metadata_roster(function: &PsiOptimizationFunction) -> bool {
    !function.structural_places.is_empty()
        && function.declared_places.is_empty()
        && function.structural_places.iter().all(|place| {
            matches!(place.kind, StructuralPlaceKind::ProviderAttachment { attachment, .. }
                if function.attachment == Some(attachment))
        })
        && !function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::EstablishByteSequenceLiteral { .. }
                )
            })
}

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
    function.blocks.iter().flat_map(|block| &block.nodes).find(|node| {
        matches!(&node.operation, AbstractOperation::CallStructuralScalar { psi_operation, structural_arguments, .. }
            | AbstractOperation::CallUnit { psi_operation, structural_arguments, .. }
            | AbstractOperation::CallStructural { psi_operation, structural_arguments, .. }
            | AbstractOperation::BoundaryCall { psi_operation, structural_arguments, .. }
            if *psi_operation == call && structural_arguments.iter().any(|argument| argument.place == source))
    })?;
    declaration_producer(function, source)
}

/// Rejoin one literal independently of unrelated storage or block arrangement.
/// Whole-unit validation checks producer dominance and borrow availability.
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
        declaration_ordinal,
        structural_type: identity,
    } = declaration.kind
    else {
        return None;
    };
    let ordinal = function
        .structural_places
        .iter()
        .filter(|place| matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. }))
        .position(|place| place.id == source)?;
    if usize::try_from(declaration_ordinal).ok()? != ordinal {
        return None;
    }
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
        || structural_type.shape
            != terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            )
    {
        return None;
    }
    Some((operation, identity))
}
