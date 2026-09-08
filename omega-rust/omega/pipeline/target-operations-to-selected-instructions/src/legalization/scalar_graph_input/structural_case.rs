//! Input-only case custody. Construction and replay produce separate outputs.
use super::*;
use semantic_vocabulary::{OperationId, PlaceId};
use terminal_psi::StructuralOperationResult;

pub(in crate::legalization) fn source_result(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Result<(OperationId, &StructuralOperationResult), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let mut matching = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            AbstractOperation::BoundaryCall {
                psi_operation,
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } if result.place == source => Some((*psi_operation, result)),
            _ => None,
        });
    let result = matching.next().ok_or(invalid.clone())?;
    if matching.next().is_some() {
        return Err(invalid);
    }
    Ok(result)
}

pub(super) fn validate(
    node: &OptimizationNode,
    function: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let AbstractOperation::StructuralCase { source, cases } = &node.operation else {
        return Err(invalid);
    };
    source_result(function, *source)?;
    if !node.provenance.is_empty()
        || !node.fuel.is_empty()
        || !node.definitions.is_empty()
        || cases.len() != node.successors.len()
    {
        return Err(invalid);
    }
    for (case, edge) in cases.iter().zip(&node.successors) {
        if edge.psi_edge != case.psi_edge
            || edge.target != case.target
            || !edge.bindings.is_empty()
            || !edge.structural_bindings.is_empty()
            || edge.trivial_affine_discards != case.trivial_affine_discards
            || !edge.residual_affine_discards.is_empty()
            || edge.provenance != [PsiProvenance::Edge(case.psi_edge)]
            || edge.fuel.is_empty()
            || edge
                .fuel
                .iter()
                .any(|fuel| fuel.site != PsiProvenance::Edge(case.psi_edge))
        {
            return Err(invalid);
        }
        for place in &case.trivial_affine_discards {
            let (_, result) = source_result(function, *place)?;
            if result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
            {
                return Err(invalid);
            }
        }
    }
    Ok(())
}
