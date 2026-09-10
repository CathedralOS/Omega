use super::*;
use semantic_vocabulary::ScalarType;
pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
    function: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<LegalizedScalarTerminator, LegalizationError> {
    match &node.operation {
        AbstractOperation::ReturnStructural {
            psi_edge, source, ..
        } => {
            let value = if function
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *source)
            {
                LegalizedScalarReturnValue::StructuralParameter { place: *source }
            } else {
                let (defining_operation, result) =
                    scalar_graph_input::structural_case::source_result(function, *source)?;
                LegalizedScalarReturnValue::Structural {
                    defining_operation,
                    result: result.clone(),
                }
            };
            Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                edge: *psi_edge,
                value,
                fuel: node.fuel.clone(),
                effect: node.effect,
                ownership: node.ownership.clone(),
            }))
        }
        AbstractOperation::StructuralCase { .. } => {
            super::structural_case::project(node, function, plan)
        }
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
            scalar_type: ScalarType::Integer(scalar_type),
            ..
        } => Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
            edge: *psi_edge,
            value: LegalizedScalarReturnValue::Value {
                value: *value,
                scalar_type: *scalar_type,
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
        structural_bindings: edge.structural_bindings.clone(),
        fuel: edge.fuel.clone(),
    }
}
