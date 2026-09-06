//! Direct structural operands execute before their enclosing call, retaining
//! captured preorder coordinates rather than renumbering by execution order.

use super::*;

pub(super) fn for_call<'a>(
    program: &TypedTrees,
    facts: &'a CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    if !calls.iter().any(|nested| {
        nested.statement_index == call.statement_index
            && nested.call_ordinal != 0
            && facts
                .flow
                .terminal_structural_returns
                .claim_free_affine_machines
                .iter()
                .any(|target| target.state == nested.target_symbol)
    }) {
        return Some(Vec::new());
    }
    let mut output = Vec::new();
    collect(
        program,
        facts,
        machine,
        state,
        calls,
        call,
        &mut Vec::new(),
        &mut output,
    )?;
    Some(output)
}

fn collect<'a>(
    program: &TypedTrees,
    facts: &'a CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &'a [checked_trees::FlowCallFact],
    call: &checked_trees::FlowCallFact,
    active: &mut Vec<typed_trees::expression::ExpressionHandle>,
    output: &mut Vec<&'a checked_trees::FlowCallFact>,
) -> Option<()> {
    let site = crate::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let arguments = crate::call_site_argument_expressions(program, &site);
    let parameters = crate::call_target_parameters(program, call.target_symbol)?;
    let explicit_self = arguments.len()
        > parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let parameters = parameters
        .iter()
        .filter(|parameter| !parameter.is_self || explicit_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }
    let initial_count = output.len();
    for (argument, parameter) in arguments.iter().zip(&parameters) {
        if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            continue;
        }
        let ExpressionNode::Call(authored) = program.expression_table.expression(*argument) else {
            continue;
        };
        let target = facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_machines
            .iter()
            .find(|plan| plan.state == authored.target_symbol)?;
        let owner = program
            .machines()
            .iter()
            .find(|owner| owner.symbol == target.machine)?;
        let target_state = crate::find_state(program, target.state)?;
        if active.contains(argument)
            || output
                .iter()
                .any(|prior| prior.authored_expression == *argument)
            || !program.call_has_no_runtime_receiver(authored, owner, target_state)
            || !authored.machine_arguments.is_empty()
            || !authored.evidence_arguments.is_empty()
            || authored.static_requirement_dispatch.is_some()
            || authored.quotient_operation.is_some()
            || authored.private_layout_operation.is_some()
        {
            return None;
        }
        let mut matching = calls.iter().filter(|nested| {
            nested.statement_index == call.statement_index
                && nested.call_ordinal > call.call_ordinal
                && nested.authored_expression == *argument
                && nested.target_symbol == authored.target_symbol
        });
        let nested = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        active.push(*argument);
        collect(
            program, facts, machine, state, calls, nested, active, output,
        )?;
        active.pop();
        output.push(nested);
    }
    if output.len() != initial_count && arguments.iter().zip(parameters).any(|(argument, parameter)| {
        program.primitive_type_reference(parameter.type_reference).is_some()
            && !matches!(program.expression_table.expression(*argument), ExpressionNode::Name(name) if name.symbol.is_valid())
    }) {
        // Other scalar operands still belong to the call's evaluator. They
        // cannot be moved across structural operand calls without retaining
        // their own argument-position evaluation and crash/fuel ordering.
        return None;
    }
    Some(())
}
