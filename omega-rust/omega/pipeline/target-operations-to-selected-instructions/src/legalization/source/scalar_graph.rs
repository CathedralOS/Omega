//! Ordered source projection; no result-specific legalization recipes.
use super::shared::*;
use crate::legalization::scalar_graph_input::{self, Input};
use ::legalized_operations::*;
pub(super) fn derive(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarFunction, LegalizationError> {
    let input = scalar_graph_input::match_input(target, abstracted, optimized, native, plan, unit)?;
    project(input, native, plan, unit)
}
fn project(
    input: Input<'_>,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarFunction, LegalizationError> {
    let parameters = input
        .optimized
        .parameters
        .iter()
        .zip(&input.call_plan.parameters)
        .map(|(parameter, placement)| LegalizedScalarParameter {
            value: parameter.value,
            scalar_type: scalar_graph_input::u64_type(),
            definition_site: parameter.site,
            placement: placement.clone(),
        })
        .collect();
    let instructions = input
        .body
        .iter()
        .map(|node| {
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
                AbstractOperation::ExactIntegerAdd { psi_operation, obligation, left, right, .. }
                | AbstractOperation::ExactIntegerSubtract { psi_operation, obligation, left, right, .. } => {
                    let fact = unit.accepted_obligation_facts.iter().find(|fact|
                        fact.machine == input.optimized.machine && fact.operation == *psi_operation && fact.obligation == *obligation)
                        .ok_or(Error::SourceCustodyMismatch)?;
                    if !input.optimized.facts.iter().any(|fact| matches!(fact,
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
                        left: *left, right: *right, obligation: *obligation, accepted_fact: fact.identity,
                    }
                }
                _ => return Err(Error::SourceCustodyMismatch),
            };
            Ok(LegalizedScalarInstruction {
                operation,
                result,
                scalar_type: scalar_graph_input::u64_type(),
                definition_site: node.definitions[0].site,
                kind,
                fuel: node.fuel.clone(),
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    let (edge, value) = match input.returned.operation {
        AbstractOperation::ReturnUnit { psi_edge, .. } => {
            (psi_edge, LegalizedScalarReturnValue::Unit)
        }
        AbstractOperation::Return {
            psi_edge, value, ..
        } => (
            psi_edge,
            LegalizedScalarReturnValue::Value {
                value,
                scalar_type: scalar_graph_input::u64_type(),
            },
        ),
        _ => unreachable!("matched return"),
    };
    Ok(LegalizedScalarFunction {
        machine: input.target.machine,
        attachment: input.target.attachment,
        provenance: input.target.provenance.clone(),
        call_plan: input.call_plan,
        parameters,
        entry_block: input.block.id,
        blocks: vec![LegalizedScalarBlock {
            id: input.block.id,
            instructions,
            terminator: LegalizedScalarReturn {
                edge,
                value,
                fuel: input.returned.fuel.clone(),
                effect: input.returned.effect,
                ownership: input.returned.ownership.clone(),
            },
        }],
    })
}
