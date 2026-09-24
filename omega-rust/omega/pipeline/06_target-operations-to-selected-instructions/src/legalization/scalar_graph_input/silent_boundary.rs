//! Silent Unit-result boundary invocations under provider custody.
//! Provider roots remain specialization witnesses: every declared place rejoins
//! an exact literal establishment and every boundary settlement produces no
//! structural result, so the function's runtime contract is its literal inputs.
use super::{AbstractBoundaryResult, AbstractOperation, PsiOptimizationFunction, literals};
use semantic_vocabulary::StructuralPlaceKind;

pub(super) fn roster(function: &PsiOptimizationFunction) -> bool {
    let literals = function
        .structural_places
        .iter()
        .filter(|place| matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. }))
        .collect::<Vec<_>>();
    !literals.is_empty()
        && function
            .structural_places
            .iter()
            .all(|place| match place.kind {
                StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                    function.attachment == Some(attachment)
                }
                StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                    literals::declaration_producer(function, place.id).is_some()
                }
                _ => false,
            })
        && function.declared_places.len() == literals.len()
        && literals
            .iter()
            .all(|place| function.declared_places.contains(&place.id))
        && function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| matches!(node.operation, AbstractOperation::BoundaryCall { .. }))
        && function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .all(|node| match &node.operation {
                AbstractOperation::BoundaryCall { result, .. } => {
                    matches!(result, AbstractBoundaryResult::Unit)
                }
                _ => true,
            })
}
