use super::*;
pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstruction, LegalizationError> {
    let (operation, result) =
        scalar_graph_input::instruction(node).ok_or(Error::SourceCustodyMismatch)?;
    let kind = match &node.operation {
        AbstractOperation::IntegerConstant { value, .. } => {
            LegalizedScalarInstructionKind::Constant(*value)
        }
        AbstractOperation::Call {
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
            ..
        } => {
            let call_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
            LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                callee: *callee,
                arguments: arguments
                    .iter()
                    .zip(&call_plan.parameters)
                    .map(|(source, placement)| LegalizedScalarArgument {
                        source: *source,
                        placement: placement.clone(),
                    })
                    .collect(),
                result_placement: call_plan.result.clone().expect("scalar callee result"),
                call_plan,
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
            })
        }
        AbstractOperation::ExactIntegerAdd {
            psi_operation,
            obligation,
            left,
            right,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            obligation,
            left,
            right,
            ..
        } => {
            let fact = unit
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == optimized.machine
                        && fact.operation == *psi_operation
                        && fact.obligation == *obligation
                })
                .ok_or(Error::SourceCustodyMismatch)?;
            if !optimized.facts.iter().any(|fact| matches!(fact,
                        optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                        if referenced == obligation && support == psi_operation)) {
                        return Err(Error::SourceCustodyMismatch);
                    }
            LegalizedScalarInstructionKind::ExactBinary {
                operator: if matches!(node.operation, AbstractOperation::ExactIntegerAdd { .. }) {
                    LegalizedExactIntegerOperator::Add
                } else {
                    LegalizedExactIntegerOperator::Subtract
                },
                left: *left,
                right: *right,
                obligation: *obligation,
                accepted_fact: fact.identity,
            }
        }
        AbstractOperation::IntegerEqual { left, right, .. }
        | AbstractOperation::IntegerLessThan { left, right, .. }
        | AbstractOperation::IntegerLessOrEqual { left, right, .. } => {
            let predicate = match node.operation {
                AbstractOperation::IntegerEqual { .. } => LegalizedScalarComparison::Equal,
                AbstractOperation::IntegerLessThan { .. } => LegalizedScalarComparison::LessThan,
                AbstractOperation::IntegerLessOrEqual { .. } => {
                    LegalizedScalarComparison::LessOrEqual
                }
                _ => return Err(Error::SourceCustodyMismatch),
            };
            let operand_type = scalar_graph_input::value_type(optimized, *left)
                .and_then(scalar_graph_input::integer_type)
                .ok_or(Error::SourceCustodyMismatch)?;
            LegalizedScalarInstructionKind::Compare {
                predicate,
                operand_type,
                left: *left,
                right: *right,
            }
        }
        _ => return Err(Error::SourceCustodyMismatch),
    };
    Ok(LegalizedScalarInstruction {
        operation,
        result,
        scalar_type: node.definitions[0].scalar_type,
        definition_site: node.definitions[0].site,
        kind,
        fuel: node.fuel.clone(),
        effect: node.effect,
        ownership: node.ownership.clone(),
    })
}
