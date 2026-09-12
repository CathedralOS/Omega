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
            | AbstractOperation::EstablishRecord {
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

/// Resolve the exact nominal case under the independently validated root access.
pub(in crate::legalization) fn membership_tag(
    function: &PsiOptimizationFunction,
    source: PlaceId,
    case: semantic_vocabulary::StructuralCaseId,
    plan: &AbstractOperationPlan,
) -> Result<u32, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let identity = if let Ok((_, result)) = source_result(function, source) {
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
        {
            return Err(invalid);
        }
        result.structural_type
    } else {
        let parameter = function
            .structural_parameters
            .iter()
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == source)
            .ok_or(invalid.clone())?;
        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !function.entry_claims.is_empty()
        {
            return Err(invalid);
        }
        parameter.structural_type
    };
    let layout = super::aggregate_results::sum_type_layout(identity, plan)?;
    if layout.tag_byte_offset != 0
        || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
    {
        return Err(invalid);
    }
    let declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == identity)
        .ok_or(invalid.clone())?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(invalid);
    };
    cases
        .iter()
        .position(|candidate| candidate.id == case)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .ok_or(invalid)
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
