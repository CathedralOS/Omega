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
            && result(
                program,
                facts,
                machine.symbol,
                nested.authored_expression,
                &mut ShapeCollector::new(program),
            )
            .is_some()
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
        result(
            program,
            facts,
            machine.symbol,
            *argument,
            &mut ShapeCollector::new(program),
        )?;
        if active.contains(argument)
            || output
                .iter()
                .any(|prior| prior.authored_expression == *argument)
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
    Some(())
}

/// Anonymous results use the same stored-owned shape as named boundary results.
/// This selects only the source signature; build_call_operation still checks
/// the complete target contract, arguments, and ownership events.
pub(in crate::flow::terminal_unit) fn result(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    shapes: &mut ShapeCollector<'_>,
) -> Option<CheckedStructuralResultPlan> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    let return_type = crate::values::nested_structural_call_return_type(program, caller, call)?;
    let mut owners = program.machines().iter().filter_map(|owner| {
        let [state] = program.machine_states(owner) else {
            return None;
        };
        (state.symbol == call.target_symbol).then_some((owner, state))
    });
    let binders = if let Some((owner, state)) = owners.next() {
        if owners.next().is_some() {
            return None;
        }
        if !owner.supply_mode.is_boundary_declaration() {
            let mut targets = facts
                .flow
                .terminal_structural_returns
                .claim_free_affine_machines
                .iter()
                .filter(|target| target.machine == owner.symbol && target.state == state.symbol);
            let target = targets.next()?;
            return (targets.next().is_none()
                && target.result.multiplicity == Multiplicity::Affine
                && target.result.qualifications.is_empty())
            .then(|| target.result.clone());
        }
        machine_binders(program, owner)
    } else {
        Vec::new()
    };
    let CheckedBoundaryMachineResultPlan::Structural {
        type_identity,
        multiplicity: Multiplicity::Affine,
        qualifications,
    } = boundary_result_plan(program, shapes, return_type, &binders)?
    else {
        return None;
    };
    qualifications
        .is_empty()
        .then_some(CheckedStructuralResultPlan {
            type_identity,
            multiplicity: Multiplicity::Affine,
            qualifications,
        })
}
