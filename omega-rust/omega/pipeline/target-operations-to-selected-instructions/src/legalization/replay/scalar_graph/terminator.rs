use super::*;
pub(super) fn validate(
    actual: &LegalizedScalarTerminator,
    node: &optimization_unit::OptimizationNode,
) -> Result<(), LegalizationError> {
    let invalid = Error::NonCanonicalLegalizedPlan;
    match (actual, &node.operation) {
        (LegalizedScalarTerminator::Return(returned), source) => {
            if returned.fuel != node.fuel
                || returned.effect != node.effect
                || returned.ownership != node.ownership
            {
                return Err(invalid);
            }
            match (&returned.value, source) {
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
                    && ScalarType::Integer(*scalar_type) == *source_type => {}
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
        && actual.fuel == source.fuel
}
