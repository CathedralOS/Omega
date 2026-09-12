//! Exact source transfers, with edge-local fuel and no discarded ownership work.
use super::*;
pub(super) fn validate(
    node: &OptimizationNode,
    _body: &[OptimizationNode],
    function: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    match (&node.operation, &function.result) {
        (AbstractOperation::StructuralCase { .. }, _) => {
            super::structural_case::validate(node, function)
        }
        (
            AbstractOperation::ReturnStructural {
                psi_edge,
                source,
                returned_claims,
                trivial_affine_locals,
                trivial_affine_discards,
            },
            AbstractFunctionResult::Structural(declared),
        ) => {
            let (structural_type, multiplicity) = if let Some(parameter) = function
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *source)
            {
                if parameter.access != terminal_psi::StructuralAccess::Owned
                    || !(parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        || parameter.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                            && crate::structural_reference_input::primitive_array_shape(
                                parameter.structural_type,
                                &plan.structural_types,
                            )
                            .is_some())
                    || parameter.is_self
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(invalid);
                }
                (parameter.structural_type, parameter.multiplicity)
            } else {
                let (_, result) = super::structural_case::source_result(function, *source)?;
                (result.structural_type, result.multiplicity)
            };
            if structural_type != declared.structural_type
                || multiplicity != declared.multiplicity
                || !declared.qualifications.is_empty()
                || !declared.projected_qualifications.is_empty()
                || !returned_claims.is_empty()
                || !trivial_affine_locals.is_empty()
                || !trivial_affine_discards.is_empty()
            {
                return Err(invalid);
            }
            return_edge(node, *psi_edge)
        }
        (
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            },
            AbstractFunctionResult::Unit,
        ) if cleanup_actions.is_empty()
            || (super::aggregate_results::cleanup(function, cleanup_actions)
                && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )])
            || (super::read_byte::cleanup(function, cleanup_actions)
                && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )])
            || (super::unobserved_owned::body(function)
                && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )]) =>
        {
            return_edge(node, *psi_edge)
        }
        (
            AbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
                cleanup_actions,
            },
            AbstractFunctionResult::Scalar(declared),
        ) if *result == declared.value
            && *scalar_type == declared.scalar_type
            && value_type(function, *value) == Some(*scalar_type)
            && (cleanup_actions.is_empty()
                || (super::aggregate_results::cleanup(function, cleanup_actions)
                    && node.ownership
                        == [optimization_unit::OwnershipEvent::Cleanup(
                            cleanup_actions.clone(),
                        )])
                || (super::unobserved_owned::body(function)
                    && node.ownership
                        == [optimization_unit::OwnershipEvent::Cleanup(
                            cleanup_actions.clone(),
                        )])) =>
        {
            return_edge(node, *psi_edge)
        }
        (
            AbstractOperation::Jump {
                psi_edge,
                target,
                bindings,
                structural_bindings,
                trivial_affine_discards,
                residual_affine_discards,
            },
            _,
        ) if trivial_affine_discards.is_empty() && residual_affine_discards.is_empty() => {
            let [edge] = node.successors.as_slice() else {
                return Err(invalid);
            };
            if edge.psi_edge != *psi_edge
                || edge.target != *target
                || edge.bindings != *bindings
                || edge.structural_bindings != *structural_bindings
            {
                return Err(invalid);
            }
            branch_edges(node)
        }
        (
            AbstractOperation::Conditional {
                condition,
                when_true,
                when_false,
            },
            _,
        ) => {
            if value_type(function, *condition) != Some(ScalarType::Boolean) {
                return Err(invalid);
            }
            if node.successors.len() != 2 {
                return Err(invalid);
            }
            for (actual, expected) in node.successors.iter().zip([when_true, when_false]) {
                if actual.psi_edge != expected.psi_edge
                    || actual.target != expected.target
                    || actual.bindings != expected.bindings
                    || actual.structural_bindings != expected.structural_bindings
                    || !expected.trivial_affine_discards.is_empty()
                {
                    return Err(invalid);
                }
            }
            branch_edges(node)
        }
        _ => Err(invalid),
    }
}
fn return_edge(
    node: &OptimizationNode,
    edge: semantic_vocabulary::EdgeId,
) -> Result<(), LegalizationError> {
    if !node.successors.is_empty()
        || node.provenance != [PsiProvenance::Edge(edge)]
        || node.fuel.is_empty()
        || node
            .fuel
            .iter()
            .any(|fuel| fuel.site != PsiProvenance::Edge(edge))
    {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    Ok(())
}
fn branch_edges(node: &OptimizationNode) -> Result<(), LegalizationError> {
    if !node.provenance.is_empty()
        || !node.fuel.is_empty()
        || node.successors.iter().any(|edge| {
            !edge.trivial_affine_discards.is_empty()
                || !edge.residual_affine_discards.is_empty()
                || edge.provenance != [PsiProvenance::Edge(edge.psi_edge)]
                || edge.fuel.is_empty()
                || edge
                    .fuel
                    .iter()
                    .any(|fuel| fuel.site != PsiProvenance::Edge(edge.psi_edge))
        })
    {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    Ok(())
}
