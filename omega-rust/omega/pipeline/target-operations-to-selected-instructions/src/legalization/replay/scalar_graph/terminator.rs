use super::*;
pub(super) fn validate(
    actual: &LegalizedScalarTerminator,
    node: &optimization_unit::OptimizationNode,
    function: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let invalid = Error::NonCanonicalLegalizedPlan;
    match (actual, &node.operation) {
        (
            LegalizedScalarTerminator::StructuralCase { .. },
            AbstractOperation::StructuralCase { .. },
        ) => {
            super::structural_case::validate(actual, node, function, plan)?;
        }
        (LegalizedScalarTerminator::Return(returned), source) => {
            if returned.fuel != node.fuel
                || returned.effect != node.effect
                || returned.ownership != node.ownership
            {
                return Err(invalid);
            }
            match (&returned.value, source) {
                (
                    LegalizedScalarReturnValue::StructuralParameter { place },
                    AbstractOperation::ReturnStructural {
                        psi_edge, source, ..
                    },
                ) if returned.edge == *psi_edge
                    && place == source
                    && function
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == *source) => {}
                (
                    LegalizedScalarReturnValue::Structural {
                        defining_operation,
                        result,
                    },
                    AbstractOperation::ReturnStructural {
                        psi_edge, source, ..
                    },
                ) => {
                    let (producer, expected) =
                        scalar_graph_input::structural_case::source_result(function, *source)?;
                    if returned.edge != *psi_edge
                        || *defining_operation != producer
                        || result != expected
                    {
                        return Err(invalid);
                    }
                }
                (
                    LegalizedScalarReturnValue::Unit,
                    AbstractOperation::ReturnUnit { psi_edge, .. },
                ) if returned.edge == *psi_edge => {}
                (
                    LegalizedScalarReturnValue::Value { value, scalar_type },
                    AbstractOperation::Return {
                        psi_edge,
                        value: source,
                        scalar_type: source_type,
                        ..
                    },
                ) if returned.edge == *psi_edge
                    && value == source
                    && *scalar_type == *source_type
                    && scalar_graph_input::scalar_shape(*scalar_type).is_some() => {}
                _ => return Err(invalid),
            }
        }
        (
            LegalizedScalarTerminator::Jump {
                successor,
                effect,
                ownership,
            },
            AbstractOperation::Jump { .. },
        ) => {
            let [edge] = node.successors.as_slice() else {
                return Err(invalid);
            };
            if !matches_edge(successor, edge)
                || *effect != node.effect
                || *ownership != node.ownership
            {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarTerminator::Conditional {
                condition,
                when_true,
                when_false,
                effect,
                ownership,
            },
            AbstractOperation::Conditional {
                condition: source, ..
            },
        ) => {
            let [true_edge, false_edge] = node.successors.as_slice() else {
                return Err(invalid);
            };
            if condition != source
                || !matches_edge(when_true, true_edge)
                || !matches_edge(when_false, false_edge)
                || *effect != node.effect
                || *ownership != node.ownership
            {
                return Err(invalid);
            }
        }
        _ => return Err(invalid),
    }
    Ok(())
}
fn matches_edge(
    actual: &LegalizedScalarSuccessor,
    source: &optimization_unit::OptimizationEdge,
) -> bool {
    actual.edge == source.psi_edge
        && actual.target == source.target
        && actual.bindings == source.bindings
        && actual.structural_bindings == source.structural_bindings
        && actual.fuel == source.fuel
}
