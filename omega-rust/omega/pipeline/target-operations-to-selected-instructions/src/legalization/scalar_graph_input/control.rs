//! Exact source transfers, with edge-local fuel and no discarded ownership work.
use super::*;
pub(super) fn validate(
    node: &OptimizationNode,
    body: &[OptimizationNode],
    function: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    match (&node.operation, &function.result) {
        (
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            },
            AbstractFunctionResult::Unit,
        ) if cleanup_actions.is_empty() => return_edge(node, *psi_edge),
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
            && cleanup_actions.is_empty() =>
        {
            return_edge(node, *psi_edge)
        }
        (
            AbstractOperation::Jump {
                psi_edge,
                target,
                bindings,
                trivial_affine_discards,
            },
            _,
        ) if trivial_affine_discards.is_empty() => {
            let [edge] = node.successors.as_slice() else {
                return Err(invalid);
            };
            if edge.psi_edge != *psi_edge || edge.target != *target || edge.bindings != *bindings {
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
            let Some(comparison) = body.last() else {
                return Err(invalid);
            };
            if !matches!(comparison.operation,AbstractOperation::IntegerEqual {result,..} | AbstractOperation::IntegerLessThan {result,..} | AbstractOperation::IntegerLessOrEqual {result,..} if result == *condition)
            {
                return Err(invalid);
            }
            if node.successors.len() != 2 {
                return Err(invalid);
            }
            for (actual, expected) in node.successors.iter().zip([when_true, when_false]) {
                if actual.psi_edge != expected.psi_edge
                    || actual.target != expected.target
                    || actual.bindings != expected.bindings
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
