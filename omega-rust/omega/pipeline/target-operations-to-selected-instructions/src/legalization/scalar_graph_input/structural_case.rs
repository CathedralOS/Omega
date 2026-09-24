//! Input-only case custody. Construction and replay produce separate outputs.
use super::{
    AbstractOperation, AbstractOperationPlan, OptimizationNode, PsiOptimizationFunction,
    PsiProvenance,
};
use crate::LegalizationError;
use semantic_vocabulary::{OperationId, PlaceId};
use terminal_psi::StructuralOperationResult;

pub(in crate::legalization) fn source_result(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Result<(OperationId, &StructuralOperationResult), LegalizationError> {
    let invalid = LegalizationError::custody();
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
            | AbstractOperation::EstablishReference {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::EstablishElementView {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::ElementViewSubslice {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::ByteSequenceSubslice {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::StructuralLeafCopy {
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
        .ok_or(LegalizationError::custody())?;
    if parameters.next().is_some()
        || declaration.access != terminal_psi::StructuralAccess::Owned
        || declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    Ok(
        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
            block,
            declaration: declaration.clone(),
        },
    )
}

/// The owner a case dispatch inspects: a stored result or block arrival as
/// `source_owner` resolves it, otherwise the function's own owned incoming
/// parameter under the same owned-arrival contract a block arrival meets.
/// Other structural consumers keep `source_owner` and never see a parameter.
pub(in crate::legalization) fn case_source(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Result<legalized_operations::LegalizedStructuralCaseSource, LegalizationError> {
    if let Ok(owner) = source_owner(function, place) {
        return Ok(owner);
    }
    let mut parameters = function
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.place == place);
    let declaration = parameters
        .next()
        .ok_or(LegalizationError::custody())?;
    if parameters.next().is_some()
        || declaration.is_self
        || declaration.access != terminal_psi::StructuralAccess::Owned
        || declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
        || !function.entry_claims.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    Ok(
        legalized_operations::LegalizedStructuralCaseSource::Parameter {
            declaration: declaration.clone(),
        },
    )
}

/// The inspected root's structural type: a live result home under the
/// same empty-custody contract, or a readable function/block parameter.
fn source_identity(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Result<semantic_vocabulary::StructuralTypeId, LegalizationError> {
    let invalid = LegalizationError::custody();
    if let Ok((_, result)) = source_result(function, source) {
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
        {
            return Err(invalid);
        }
        return Ok(result.structural_type);
    }
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
    Ok(parameter.structural_type)
}

/// Resolve the exact nominal case under the independently validated root access.
pub(in crate::legalization) fn membership_layout(
    function: &PsiOptimizationFunction,
    source: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    case: semantic_vocabulary::StructuralCaseId,
    plan: &AbstractOperationPlan,
) -> Result<(u32, u32), LegalizationError> {
    let invalid = LegalizationError::custody();
    let identity = source_identity(function, source)?;
    let (identity, byte_offset) = if path.is_empty() {
        (identity, 0)
    } else {
        crate::structural_inputs::structural_reference_input::project(
            identity,
            path,
            &plan.structural_types,
        )
        .ok_or(invalid.clone())?
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
    let cases: &[terminal_psi::StructuralCaseDeclaration] = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Sum { cases }
        | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return Err(invalid),
    };
    cases
        .iter()
        .position(|candidate| candidate.id == case)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .map(|tag| (tag, byte_offset))
        .ok_or(invalid)
}

/// Resolve the copied leaf's byte offset inside the readable root and its
/// canonical shape. The path's endpoint type must be the declared result type
/// and the result must keep the copy's unrestricted empty custody.
/// `RuntimeIndex` segments additionally yield `(selector, stride)` pairs in
/// path order: each named dense parameter must exist and carry an integer
/// index no wider than the address model, mirroring the indexed-store
/// operand contract.
pub(in crate::legalization) fn leaf_copy_layout(
    function: &PsiOptimizationFunction,
    source: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<(u32, calling_conventions::ValueShape, Vec<(u32, u32)>), LegalizationError> {
    let invalid = LegalizationError::custody();
    if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(invalid);
    }
    let identity = source_identity(function, source)?;
    let (endpoint, byte_offset, indices) = if path.is_empty() {
        (identity, 0, Vec::new())
    } else {
        crate::structural_inputs::structural_reference_input::leaf_copy_projection(
            identity,
            path,
            &plan.structural_types,
        )
        .ok_or(invalid.clone())?
    };
    if endpoint != result.structural_type
        || super::reference_custody::contains_reference(&plan.structural_types, endpoint)
    {
        return Err(invalid);
    }
    for (selector, _) in &indices {
        let parameter = usize::try_from(*selector)
            .ok()
            .and_then(|position| function.parameters.get(position))
            .ok_or(invalid.clone())?;
        let semantic_vocabulary::ScalarType::Integer(index_type) = parameter.scalar_type else {
            return Err(invalid);
        };
        if index_type.bits() > 64 {
            return Err(invalid);
        }
    }
    let shape = crate::structural_inputs::structural_reference_input::shape(
        endpoint,
        &plan.structural_types,
    )
    .ok_or(invalid)?;
    Ok((byte_offset, shape, indices))
}

pub(super) fn validate(
    node: &OptimizationNode,
    function: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::custody();
    let AbstractOperation::StructuralCase { source, cases } = &node.operation else {
        return Err(invalid);
    };
    case_source(function, *source)?;
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
        // A case edge's discards are admitted by the same rule as every other
        // edge's and every return's: a plain affine result, block arrival, or
        // the function's own owned parameter — which is what a dispatch on an
        // owned parameter leaves behind on each arm.
        let actions = case
            .trivial_affine_discards
            .iter()
            .copied()
            .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
            .collect::<Vec<_>>();
        if !super::aggregate_results::cleanup(function, &actions) {
            return Err(invalid);
        }
    }
    Ok(())
}
