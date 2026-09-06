use super::super::leaves::{exact_edge_fuel, exact_operation_fuel};
use super::super::shared::*;
use super::grammar::MatchedSequence;

pub(super) fn project(
    function: usize,
    matched: MatchedSequence<'_>,
) -> Result<LegalizedScalarCallUnitFunction, LegalizationError> {
    let operations = matched
        .operations
        .iter()
        .zip(matched.nodes)
        .map(|(operation, node)| match operation {
            TargetUnitOperation::IntegerConstant { .. } => {
                let TargetUnitOperation::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type,
                    value,
                } = operation
                else {
                    unreachable!()
                };
                Ok(LegalizedScalarCallUnitOperation::Constant(
                    LegalizedScalarCallUnitConstant {
                        operation: *psi_operation,
                        result: *result,
                        scalar_type: *scalar_type,
                        value: *value,
                        definition_site: node.definitions[0].site,
                        fuel: exact_operation_fuel(node, *psi_operation, function)
                            .expect("matched fuel"),
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    },
                ))
            }
            TargetUnitOperation::ScalarCall { .. } => {
                let TargetUnitOperation::ScalarCall {
                    psi_operation,
                    callee,
                    call_plan,
                    result_home,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                } = operation
                else {
                    unreachable!()
                };
                Ok(LegalizedScalarCallUnitOperation::Call(
                    LegalizedScalarCallUnitCall {
                        operation: *psi_operation,
                        callee: *callee,
                        call_plan: call_plan.clone(),
                        result_home: *result_home,
                        result_definition_site: node.definitions[0].site,
                        arguments: arguments.iter().map(project_argument).collect(),
                        requirement_obligations: requirement_obligations.clone(),
                        crash_continuations: crash_continuations.clone(),
                        fuel: exact_operation_fuel(node, *psi_operation, function)
                            .expect("matched fuel"),
                        effect: node.effect,
                        ownership: node.ownership.clone(),
                    },
                ))
            }
            _ => Err(Error::UnsupportedSourceShape { function }),
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    let return_node = matched.return_node;
    Ok(LegalizedScalarCallUnitFunction {
        machine: matched.target.machine,
        attachment: matched.attachment,
        provenance: matched.target.provenance.clone(),
        recipe: ScalarCallUnitLegalizationRecipe::OrderedU64RegisterCallsThenReturnUnitV1,
        entry_block: matched.block.id,
        operations,
        return_edge: matched.return_edge,
        return_fuel: exact_edge_fuel(return_node, matched.return_edge, function)?,
        return_effect: return_node.effect,
        return_ownership: return_node.ownership.clone(),
    })
}

fn project_argument(
    argument: &target_operations::TargetUnitScalarCallArgument,
) -> LegalizedScalarCallUnitArgument {
    LegalizedScalarCallUnitArgument {
        parameter_index: argument.parameter_index,
        source: argument.source,
        placement: argument.placement.clone(),
    }
}
