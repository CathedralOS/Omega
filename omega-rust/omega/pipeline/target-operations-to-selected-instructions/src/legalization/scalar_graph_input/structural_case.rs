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
            AbstractOperation::EstablishScalarArray {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::EstablishScalarCase {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                result,
                ..
            } if result.place == source => Some((*psi_operation, result)),
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

pub(in crate::legalization) fn source_owner(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Result<legalized_operations::LegalizedStructuralCaseSource, LegalizationError> {
    if let Ok((operation, result)) = source_result(function, place) {
        return Ok(
            legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                operation,
                result: result.clone(),
            },
        );
    }
    let mut parameters = function.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    });
    let (block, declaration) = parameters
        .next()
        .ok_or(LegalizationError::SourceCustodyMismatch)?;
    if parameters.next().is_some()
        || declaration.access != terminal_psi::StructuralAccess::Owned
        || declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    Ok(
        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
            block,
            declaration: declaration.clone(),
        },
    )
}

pub(super) fn validate(
    node: &OptimizationNode,
    function: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let AbstractOperation::StructuralCase { source, cases } = &node.operation else {
        return Err(invalid);
    };
    source_owner(function, *source)?;
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
            let plain_affine = match source_owner(function, *place)? {
                legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                    result,
                    ..
                } => {
                    result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                }
                legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                    declaration,
                    ..
                } => declaration.multiplicity == terminal_psi::StructuralMultiplicity::Affine,
            };
            if !plain_affine {
                return Err(invalid);
            }
        }
    }
    Ok(())
}
