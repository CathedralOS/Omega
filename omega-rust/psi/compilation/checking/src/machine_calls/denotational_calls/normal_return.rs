//! Conditional equality of concrete value-only calls during validation.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// Select a call whose normal result is determined by its runtime inputs.
///
/// This is not a fact-call certificate: it proves neither termination nor
/// precondition discharge nor absence of crashes. Executable comparisons can
/// use the same result conditional on both evaluations returning normally.
/// Authored fact denotation still owes its existing later totality checks.
/// The caller must separately compare exact input values and invalidate their
/// dependencies across writes; equal target spelling is never sufficient.
/// Both summaries must belong to this same typed program.
pub fn normal_return_call_candidate<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &TableCallExpression,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
) -> Result<(&'program Machine, &'program State), &'static str> {
    let (machine, state) = plain_value_call_target(program, call)
        .ok_or("the call is not one exact concrete free value call")?;
    if !super::has_pure_effect_closure(
        program,
        machine.symbol,
        state.symbol,
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_mutable),
        operational,
        service_reaches,
    ) || !super::has_observation_free_checked_closure(program, machine.symbol, operational)
    {
        return Err("the call closure has effects, observations, or unresolved targets");
    }
    if !primitive_value_closure(program, machine.symbol, operational) {
        return Err("the call closure lacks exact value-input or builtin operator custody");
    }
    Ok((machine, state))
}

pub(crate) fn plain_value_call_target<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<(&'program Machine, &'program State)> {
    if !call.target_symbol.is_valid()
        || call.receiver.is_valid()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    let mut targets = program.machine_states_named(call.target_symbol);
    let (machine, state) = targets.next()?;
    if targets.next().is_some()
        || program.machine_states(machine).first()?.symbol != state.symbol
        || !primitive_value_machine(program, machine)
        || program.state_parameters(state).len()
            != program
                .expression_table
                .expression_handles(call.arguments)
                .len()
    {
        return None;
    }
    Some((machine, state))
}

fn primitive_value_machine(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    machine.symbol.is_valid()
        && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        && machine.body_is_present
        && machine.attached_data.is_none()
        && machine.lifetime_parameters.is_empty()
        && program.machine_type_parameters(machine).is_empty()
        && program.machine_states(machine).iter().all(|state| {
            state.symbol.is_valid()
                && program
                    .primitive_type_reference(state.return_type)
                    .is_some()
                && program.state_parameters(state).iter().all(|parameter| {
                    parameter.symbol.is_valid()
                        && !parameter.is_mutable
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                })
        })
}

/// The shared observation/effect closure owns purity. This additional rung
/// binds reads to explicit value inputs and operator meaning: operational call
/// summaries do not include implementations of arbitrary infix operators.
fn primitive_value_closure(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    operational: &flow_effects::OperationalPlan,
) -> bool {
    let mut pending = vec![machine_symbol];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let mut machines = program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == current);
        let Some(machine) = machines.next() else {
            return false;
        };
        if machines.next().is_some() || !primitive_value_machine(program, machine) {
            return false;
        }
        for state in program.machine_states(machine) {
            let mut available = program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let expression_is_closed = |expression| {
                    closed_expression(program, machine, state, expression, &available, 0)
                };
                let allowed = match statement {
                    StatementNode::Expression(_) | StatementNode::Transition(_) => program
                        .statement_table
                        .expression_roots(statement)
                        .iter()
                        .all(|(_, expression)| expression_is_closed(*expression)),
                    StatementNode::LocalData(local) => {
                        let allowed = local.symbol.is_valid()
                            && !local.is_mutable
                            && expression_is_closed(local.initial_value);
                        available.push(local.symbol);
                        allowed
                    }
                    // Resultless work and mutable storage are not part of this
                    // value-only rung, even if a broader effect proof exists.
                    StatementNode::Call(_)
                    | StatementNode::Assignment(_)
                    | StatementNode::RootBinding(_)
                    | StatementNode::Assembly(_)
                    | StatementNode::AssemblyFact(_) => false,
                };
                if !allowed {
                    return false;
                }
            }
        }
        let mut summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == current);
        let Some(summary) = summaries.next() else {
            return false;
        };
        if summaries.next().is_some() {
            return false;
        }
        for state in operational.states.span_or_empty(summary.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if !call.target_machine_symbol.is_valid() {
                    return false;
                }
                pending.push(call.target_machine_symbol);
            }
        }
    }
    true
}

fn closed_expression(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    available: &[SymbolHandle],
    depth: usize,
) -> bool {
    if !expression.is_valid() || depth >= 128 {
        return false;
    }
    let child =
        |expression| closed_expression(program, machine, state, expression, available, depth + 1);
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) => true,
        ExpressionNode::Name(path) => {
            path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && available.contains(&path.symbol)
        }
        ExpressionNode::Binary(binary) => {
            crate::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
                && child(binary.left)
                && child(binary.right)
        }
        ExpressionNode::Call(call) => {
            plain_value_call_target(program, call).is_some()
                && program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| child(*argument))
        }
        // Broader projections, adapters and reference reads need their own
        // exact read/meaning custody, not a syntactic claim of purity.
        _ => false,
    }
}
