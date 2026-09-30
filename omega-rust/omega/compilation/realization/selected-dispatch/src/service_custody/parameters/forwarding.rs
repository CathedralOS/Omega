//! Rejoin each ordinary Binding argument to its exact receiving receipt.
//! This checks service identity, not loan formation or restoration. Ordinary
//! structural-call custody still checks each authored operand and its lifetime.

use super::{CheckedTrees, CheckedUnitStructuralParameterPlan, UnitState, unit_states};
use checked_trees::CheckedStructuralAccess;

pub(super) fn validate(
    checked: &CheckedTrees,
    parameter: &CheckedUnitStructuralParameterPlan,
    receipt: &checked_trees::CheckedFusedServiceParameterReceipt,
    plan: &UnitState<'_>,
) -> Result<(), &'static str> {
    let source_index = plan
        .structural_parameters
        .iter()
        .position(|candidate| std::ptr::eq(candidate, parameter))
        .and_then(|position| u32::try_from(position).ok())
        .ok_or("the forwarding source lost its exact structural ordinal")?;
    let mut flow_states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| state.machine_symbol == plan.machine && state.state_symbol == plan.state);
    let flow = flow_states
        .next()
        .ok_or("the forwarding source lost its flow state")?;
    if flow_states.next().is_some() {
        return Err("the forwarding source duplicates its flow state");
    }
    for call in plan
        .operations
        .iter()
        .filter_map(|operation| operation.call())
        .filter(|call| !call.is_boundary())
    {
        for (argument_index, argument) in call.structural_arguments.iter().enumerate() {
            if argument.source_parameter_index() != Some(source_index) {
                continue;
            }
            if !argument.path.is_empty()
                || argument.type_identity != parameter.type_identity
                || !access_preserved(parameter.access, argument.access)
            {
                return Err("the forwarding edge substituted its whole binding or access");
            }
            let mut calls = checked
                .facts
                .flow
                .control
                .calls
                .span_or_empty(flow.calls)
                .iter()
                .filter(|source| {
                    source.statement_index == call.coordinate.statement_index as usize
                        && source.call_ordinal == call.coordinate.call_ordinal as usize
                });
            let source = calls
                .next()
                .ok_or("the forwarding edge lost its source call")?;
            if calls.next().is_some() || source.target_symbol != call.target.state {
                return Err("the forwarding edge substituted its exact source call");
            }
            if !checked
                .facts
                .borrow
                .argument_accesses
                .span_or_empty(source.accesses)
                .iter()
                .any(|access| {
                    access.root_symbol == receipt.source_parameter && access.segments.is_empty()
                })
            {
                return Err("the forwarding edge lost its captured source binding");
            }
            let mut targets = unit_states(checked).filter(|candidate| {
                candidate.machine == call.target.machine && candidate.state == call.target.state
            });
            let target = targets
                .next()
                .ok_or("the forwarding edge lost its checked target state")?;
            if targets.next().is_some() {
                return Err("the forwarding edge duplicates its checked target state");
            }
            let target_parameter = target
                .structural_parameters
                .get(argument_index)
                .ok_or("the forwarding edge lost its receiving structural ordinal")?;
            let target_receipt = target_parameter
                .fused_service_erasure
                .as_ref()
                .ok_or("the forwarding target lost its Binding receipt")?;
            if target_parameter.type_identity != parameter.type_identity
                || target_parameter.qualifications != parameter.qualifications
                || target_parameter.access != argument.access
                || target_receipt.requirement != receipt.requirement
            {
                return Err(
                    "the forwarding target substituted carrier, qualifications, access or requirement",
                );
            }
            // Every target's complete receipt is replayed by the state roster.
            // Rejoin its source here as well, so ordinal or symbol substitution
            // cannot redirect this edge to another parameter with the same type.
            let target_machine = checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == target.machine)
                .ok_or("the forwarding target lost its typed machine")?;
            let target_state = checked
                .machine_states(target_machine)
                .iter()
                .find(|state| state.symbol == target.state)
                .ok_or("the forwarding target lost its typed state")?;
            let target_source = checked
                .state_parameters(target_state)
                .get(target_parameter.position as usize)
                .ok_or("the forwarding target substituted its authored parameter position")?;
            let operand = source_operand(
                checked,
                plan,
                source,
                target_state,
                target_parameter.position,
            )
            .ok_or("the forwarding edge lost its exact authored argument position")?;
            if !checked.expression_table.expression_is_valid(operand) {
                return Err("the forwarding edge has a stale authored argument");
            }
            let operand = match checked.expression_table.expression(operand) {
                typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                    if CheckedStructuralAccess::from(borrow.access) != argument.access {
                        return Err("the forwarding edge substituted its authored borrow access");
                    }
                    borrow.target
                }
                _ => operand,
            };
            if !checked.expression_table.expression_is_valid(operand)
                || !matches!(checked.expression_table.expression(operand),
                typed_trees::expression::ExpressionNode::Name(path)
                if path.symbol == receipt.source_parameter
                    && path.head_symbol == receipt.source_parameter
                    && path.members.count() == 1)
            {
                return Err(
                    "the forwarding edge substituted the binding at its authored argument position",
                );
            }
            let target_carrier = checked
                .bound_service_parameter_carrier(target_source.type_reference)
                .map_err(|_| "the forwarding target has an invalid Binding carrier")?
                .ok_or("the forwarding target is not a Binding carrier")?;
            if target_source.symbol != target_receipt.source_parameter
                || target_carrier.carrier_type_identity != target_receipt.carrier_type_identity
                || target_carrier.base_type_identity != target_parameter.type_identity
                || target_carrier.requirement != target_receipt.requirement
                || target_carrier.access != argument.access
            {
                return Err("the forwarding target receipt differs from its exact typed source");
            }
        }
    }
    Ok(())
}

/// Flow capture supplies nested expression identity; statement-root calls keep
/// their exact statement coordinate. Map the authored formal position before
/// examining its operand, so equal-typed Binding arguments cannot be swapped.
fn source_operand(
    checked: &CheckedTrees,
    plan: &UnitState<'_>,
    source: &checked_trees::FlowCallFact,
    target: &typed_trees::state::State,
    formal_position: u32,
) -> Option<typed_trees::expression::ExpressionHandle> {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    let expression_arguments = |expression| {
        if !checked.expression_table.expression_is_valid(expression) {
            return None;
        }
        let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
            return None;
        };
        (call.target_symbol == source.target_symbol)
            .then_some(checked.expression_table.expression_handles(call.arguments))
    };
    let arguments = if source.authored_expression.is_valid() {
        expression_arguments(source.authored_expression)?
    } else {
        if source.call_ordinal != 0 {
            return None;
        }
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == plan.machine)?;
        let state = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == plan.state)?;
        match checked
            .statement_table
            .statements(state.statement_nodes)
            .get(source.statement_index)?
        {
            StatementNode::Call(call) if call.target_symbol == source.target_symbol => {
                checked.statement_table.expression_handles(call.arguments)
            }
            StatementNode::LocalData(local) => expression_arguments(local.initial_value)?,
            StatementNode::Assignment(assignment) => expression_arguments(assignment.value)?,
            StatementNode::Expression(expression) => expression_arguments(*expression)?,
            StatementNode::Transition(transition) => {
                let TransitionTargetNode::Named {
                    path, arguments, ..
                } = checked.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                if path.symbol != source.target_symbol {
                    return None;
                }
                checked.statement_table.expression_handles(*arguments)
            }
            _ => return None,
        }
    };
    let parameters = checked.state_parameters(target);
    let nonself_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let explicit_self = arguments.len() > nonself_count;
    if arguments.len()
        != if explicit_self {
            parameters.len()
        } else {
            nonself_count
        }
    {
        return None;
    }
    let mut explicit_position = 0;
    for (position, parameter) in parameters.iter().enumerate() {
        if parameter.is_self && !explicit_self {
            continue;
        }
        if position == formal_position as usize {
            return arguments.get(explicit_position).copied();
        }
        explicit_position += 1;
    }
    None
}

fn access_preserved(source: CheckedStructuralAccess, argument: CheckedStructuralAccess) -> bool {
    use CheckedStructuralAccess::{MutableBorrow, Owned, SharedBorrow};
    matches!(
        (source, argument),
        (Owned, Owned | SharedBorrow | MutableBorrow)
            | (MutableBorrow, MutableBorrow | SharedBorrow)
            | (SharedBorrow, SharedBorrow)
    )
}
