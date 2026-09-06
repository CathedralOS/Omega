//! A trailing call returns Unit only when its exact declaration does.

use super::*;

pub fn unit_return_call_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if !unit_type(program, state.return_type)
        || !program.expression_table.expression_is_valid(expression)
        || !matches!(
            program.statement_table.statements(state.statement_nodes).last(),
            Some(typed_trees::statement::StatementNode::Expression(tail)) if *tail == expression
        )
    {
        return false;
    }
    call_returns_unit(program, machine, expression)
}

pub(in crate::calls) fn call_returns_unit(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    if !call.target_symbol.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return false;
    }
    let mut results = program.machines().iter().flat_map(|target| {
        program
            .machine_states(target)
            .iter()
            .filter_map(move |target_state| {
                (target_state.symbol == call.target_symbol).then_some((target, target_state))
            })
    });
    if let Some((target, target_state)) = results.next() {
        return results.next().is_none()
            && unit_type(program, target_state.return_type)
            && (target_state.return_type.is_valid()
                || !has_inferred_value_return(program, target));
    }
    let requirement = match program.machine_parameter_signature(call.target_symbol) {
        Some((owner, signature)) if owner.symbol == machine.symbol => signature.symbol,
        Some(_) => return false,
        None => call.target_symbol,
    };
    let mut results = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .filter(|signature| signature.symbol == requirement)
        .map(|signature| signature.return_type);
    results
        .next()
        .is_some_and(|result| unit_type(program, result))
        && results.next().is_none()
}

fn has_inferred_value_return(program: &TypedTrees, machine: &Machine) -> bool {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    program.machine_states(machine).iter().any(|state| {
        (state.return_type.is_valid() && !unit_type(program, state.return_type))
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    let StatementNode::Transition(transition) = statement else {
                        return false;
                    };
                    [transition.target, transition.continuation]
                        .iter()
                        .any(|target| {
                            target.is_valid()
                                && matches!(
                                    program.statement_table.transition_target(*target),
                                    TransitionTargetNode::Value(_)
                                )
                        })
                })
    })
}

fn unit_type(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Unit => return true,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return false,
        }
    }
}
