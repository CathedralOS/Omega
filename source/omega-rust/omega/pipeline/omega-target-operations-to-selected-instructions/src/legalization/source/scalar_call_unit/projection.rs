use super::super::leaves::{exact_edge_fuel, exact_operation_fuel};
use super::super::shared::*;
use super::grammar::MatchedChain;

pub(super) fn project(
    function: usize,
    matched: MatchedChain<'_>,
) -> Result<LegalizedScalarCallUnitFunction, LegalizationError> {
    let constants = std::array::from_fn(|index| {
        let TargetUnitOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } = matched.target_constants[index]
        else {
            unreachable!()
        };
        let node = matched.nodes[index];
        LegalizedScalarCallUnitConstant {
            operation: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            value: *value,
            definition_site: node.definitions[0].site,
            fuel: exact_operation_fuel(node, *psi_operation, function).expect("matched fuel"),
            effect: node.effect,
            ownership: node.ownership.clone(),
        }
    });
    let calls = std::array::from_fn(|index| {
        let TargetUnitOperation::ScalarCall {
            psi_operation,
            callee,
            call_plan,
            result_home,
            arguments,
            requirement_obligations,
            crash_continuations,
        } = matched.target_calls[index]
        else {
            unreachable!()
        };
        let node = matched.nodes[index + 2];
        let [left, right] = arguments.as_slice() else {
            unreachable!()
        };
        LegalizedScalarCallUnitCall {
            operation: *psi_operation,
            callee: *callee,
            call_plan: call_plan.clone(),
            result_home: *result_home,
            result_definition_site: node.definitions[0].site,
            arguments: [project_argument(left), project_argument(right)],
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
            fuel: exact_operation_fuel(node, *psi_operation, function).expect("matched fuel"),
            effect: node.effect,
            ownership: node.ownership.clone(),
        }
    });
    let return_node = matched.nodes[5];
    Ok(LegalizedScalarCallUnitFunction {
        machine: matched.target.machine,
        attachment: matched.attachment,
        provenance: matched.target.provenance.clone(),
        recipe:
            ScalarCallUnitLegalizationRecipe::U64EqualityConditionalThreeCallChainThenReturnUnitV1,
        entry_block: matched.block.id,
        constants,
        calls,
        return_edge: matched.return_edge,
        return_fuel: exact_edge_fuel(return_node, matched.return_edge, function)?,
        return_effect: return_node.effect,
        return_ownership: return_node.ownership.clone(),
    })
}

fn project_argument(
    argument: &omega_target_operations::TargetUnitScalarCallArgument,
) -> LegalizedScalarCallUnitArgument {
    LegalizedScalarCallUnitArgument {
        parameter_index: argument.parameter_index,
        source: argument.source,
        placement: argument.placement.clone(),
    }
}
