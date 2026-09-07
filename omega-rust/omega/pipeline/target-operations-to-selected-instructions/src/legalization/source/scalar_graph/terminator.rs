use super::*;
pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
) -> Result<LegalizedScalarTerminator, LegalizationError> {
    match &node.operation {
        AbstractOperation::ReturnUnit { psi_edge, .. } => {
            Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                edge: *psi_edge,
                value: LegalizedScalarReturnValue::Unit,
                fuel: node.fuel.clone(),
                effect: node.effect,
                ownership: node.ownership.clone(),
            }))
        }
        AbstractOperation::Return {
            psi_edge,
            value,
            scalar_type,
            ..
        } => Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
            edge: *psi_edge,
            value: LegalizedScalarReturnValue::Value {
                value: *value,
                scalar_type: scalar_graph_input::integer_type(*scalar_type)
                    .ok_or(Error::SourceCustodyMismatch)?,
            },
            fuel: node.fuel.clone(),
            effect: node.effect,
            ownership: node.ownership.clone(),
        })),
        AbstractOperation::Jump { .. } => {
            let [edge] = node.successors.as_slice() else {
                return Err(Error::SourceCustodyMismatch);
            };
            Ok(LegalizedScalarTerminator::Jump {
                successor: successor(edge),
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        }
        AbstractOperation::Conditional { condition, .. } => {
            let [when_true, when_false] = node.successors.as_slice() else {
                return Err(Error::SourceCustodyMismatch);
            };
            Ok(LegalizedScalarTerminator::Conditional {
                condition: *condition,
                when_true: successor(when_true),
                when_false: successor(when_false),
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        }
        _ => Err(Error::SourceCustodyMismatch),
    }
}
fn successor(edge: &optimization_unit::OptimizationEdge) -> LegalizedScalarSuccessor {
    LegalizedScalarSuccessor {
        edge: edge.psi_edge,
        target: edge.target,
        bindings: edge.bindings.clone(),
        fuel: edge.fuel.clone(),
    }
}
