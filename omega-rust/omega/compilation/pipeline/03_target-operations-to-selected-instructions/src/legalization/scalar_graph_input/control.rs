//! Exact source transfers, with edge-local fuel and no discarded ownership work.
use super::{
    AbstractFunctionResult, AbstractOperation, OptimizationNode, PsiOptimizationFunction,
    PsiProvenance, ScalarType,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::value_type;
pub(super) fn validate(
    node: &OptimizationNode,
    _body: &[OptimizationNode],
    function: &PsiOptimizationFunction,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    match (&node.operation, &function.result) {
        (AbstractOperation::ExternalCompletion { psi_edge, .. }, _)
        | (AbstractOperation::Crash { psi_edge, .. }, _) => terminal_edge(node, *psi_edge),
        (AbstractOperation::StructuralCase { .. }, _) => {
            super::structural_case::validate(node, function, structural_types)
        }
        (
            AbstractOperation::ReturnStructural {
                psi_edge,
                source,
                returned_claims,
                trivial_affine_locals,
                cleanup_actions,
            },
            AbstractFunctionResult::Structural(declared),
        ) => {
            if terminal_semantics::descriptor_results::is_descriptor(
                structural_types.iter(),
                declared.structural_type,
            ) {
                if !super::byte_views::descriptor_return(function, *source, structural_types)
                    || !returned_claims.is_empty()
                    || !trivial_affine_locals.is_empty()
                    || !super::aggregate_results::cleanup(
                        function,
                        &cleanup_actions,
                        structural_types,
                    )
                {
                    return Err(LegalizationError::custody());
                }
                return terminal_edge(node, *psi_edge);
            }
            // Result membership was established by Psi at this edge. Native
            // custody resolves the exact source carrier, not an equality
            // between its stored labels and the declared postcondition.
            let (structural_type, multiplicity) =
                if let Some(parameter) = function.structural_parameter(*source) {
                    if parameter.access != terminal_psi::StructuralAccess::Owned {
                        return Err(LegalizationError::custody());
                    }
                    (parameter.structural_type, parameter.multiplicity)
                } else {
                    match super::structural_case::source_owner(function, *source)? {
                    // The returned claims leave with the value; ownership
                    // replay already matched them to the source's claims.
                    legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                        result,
                        ..
                    } => (result.structural_type, result.multiplicity),
                    legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                        declaration,
                        ..
                    } => (declaration.structural_type, declaration.multiplicity),
                    // `source_owner` never resolves a function parameter.
                    legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
                    | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                        ..
                    } => {
                        return Err(LegalizationError::custody());
                    }
                }
                };
            if structural_type != declared.structural_type
                || multiplicity != declared.multiplicity
                || !trivial_affine_locals.is_empty()
                || !super::aggregate_results::cleanup(function, &cleanup_actions, structural_types)
            {
                return Err(LegalizationError::custody());
            }
            terminal_edge(node, *psi_edge)
        }
        (
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            },
            AbstractFunctionResult::Unit,
        ) if cleanup_actions.is_empty()
            || (super::aggregate_results::cleanup(function, cleanup_actions, structural_types)
                && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )])
            || (super::read_byte::cleanup(function, cleanup_actions)
                && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )]) =>
        {
            terminal_edge(node, *psi_edge)
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
                || (super::aggregate_results::cleanup(
                    function,
                    cleanup_actions,
                    structural_types,
                ) && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )])) =>
        {
            terminal_edge(node, *psi_edge)
        }
        (
            AbstractOperation::Jump {
                psi_edge,
                target,
                bindings,
                structural_bindings,
                cleanup_actions,
            },
            _,
        ) => {
            let [edge] = node.successors.as_slice() else {
                return Err(LegalizationError::custody());
            };
            if edge.psi_edge != *psi_edge
                || edge.target != *target
                || edge.bindings != *bindings
                || edge.structural_bindings != *structural_bindings
                || edge.cleanup_actions != *cleanup_actions
            {
                return Err(LegalizationError::custody());
            }
            branch_edges(node, function, structural_types)
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
                return Err(LegalizationError::custody());
            }
            if node.successors.len() != 2 {
                return Err(LegalizationError::custody());
            }
            for (actual, expected) in node.successors.iter().zip([when_true, when_false]) {
                if actual.psi_edge != expected.psi_edge
                    || actual.target != expected.target
                    || actual.bindings != expected.bindings
                    || actual.structural_bindings != expected.structural_bindings
                    || actual.cleanup_actions != expected.cleanup_actions
                {
                    return Err(LegalizationError::custody());
                }
            }
            branch_edges(node, function, structural_types)
        }
        _ => Err(LegalizationError::custody()),
    }
}

fn terminal_edge(
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
        return Err(LegalizationError::custody());
    }
    Ok(())
}
fn branch_edges(
    node: &OptimizationNode,
    function: &PsiOptimizationFunction,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    if !node.provenance.is_empty()
        || !node.fuel.is_empty()
        || node.successors.iter().any(|edge| {
            !super::aggregate_results::cleanup(function, &edge.cleanup_actions, structural_types)
                || edge.provenance != [PsiProvenance::Edge(edge.psi_edge)]
                || edge.fuel.is_empty()
                || edge
                    .fuel
                    .iter()
                    .any(|fuel| fuel.site != PsiProvenance::Edge(edge.psi_edge))
        })
    {
        return Err(LegalizationError::custody());
    }
    Ok(())
}
