//! Shared eligibility for calls whose result is used denotationally.
//!
//! Quotient operations and fact-position call projections must agree on the
//! operational meaning of "pure and unconditionally terminating".  This
//! module consumes the existing whole-program summaries; it never performs a
//! second expression-local effect inference.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod normal_return;
pub(crate) use normal_return::normal_return_call_candidate;

pub(crate) fn has_observation_free_checked_closure(
    program: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    operational: &flow_effects::OperationalPlan,
) -> bool {
    let mut pending = vec![machine_symbol];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == current)
        else {
            return false;
        };
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !machine.body_is_present
            || !program.machine_owned_data(machine).is_empty()
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty()
        {
            return false;
        }
        for state in program.machine_states(machine) {
            if program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_mutable)
            {
                return false;
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                use typed_trees::statement::StatementNode;
                let allowed = match statement {
                    StatementNode::AssemblyFact(_) | StatementNode::Assignment(_) => false,
                    StatementNode::Expression(expression) => {
                        expression_is_fact_observation_free(program, *expression)
                    }
                    StatementNode::LocalData(local) => {
                        !local.is_mutable
                            && expression_is_fact_observation_free(program, local.initial_value)
                    }
                    StatementNode::Call(call) => {
                        !call.receiver_symbol.is_valid()
                            && program
                                .expression_table
                                .expression_handles(call.arguments)
                                .iter()
                                .all(|argument| {
                                    expression_is_fact_observation_free(program, *argument)
                                })
                    }
                    StatementNode::Transition(transition) => {
                        transition_is_fact_observation_free(program, transition)
                    }
                };
                if !allowed {
                    return false;
                }
            }
        }
        let summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == current)
            .collect::<Vec<_>>();
        let [summary] = summaries.as_slice() else {
            return false;
        };
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

fn transition_is_fact_observation_free(
    program: &TypedTrees,
    transition: &typed_trees::statement::TableTransition,
) -> bool {
    use typed_trees::statement::{TransitionGuardNode, TransitionTargetNode};
    let guard = match transition.guard {
        TransitionGuardNode::Always => true,
        TransitionGuardNode::When(expression) => {
            expression_is_fact_observation_free(program, expression)
        }
    };
    let target = |handle| match program.statement_table.transition_target(handle) {
        TransitionTargetNode::Named { arguments, .. } => program
            .expression_table
            .expression_handles(*arguments)
            .iter()
            .all(|argument| expression_is_fact_observation_free(program, *argument)),
        TransitionTargetNode::Value(expression) => {
            expression_is_fact_observation_free(program, *expression)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => true,
    };
    guard && target(transition.target) && target(transition.continuation)
}

fn expression_is_fact_observation_free(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(_) | ExpressionNode::Borrow(_) => false,
        ExpressionNode::Binary(binary) => {
            expression_is_fact_observation_free(program, binary.left)
                && expression_is_fact_observation_free(program, binary.right)
        }
        ExpressionNode::Cast(cast) => expression_is_fact_observation_free(program, cast.value),
        ExpressionNode::Call(call) => {
            !call.receiver.is_valid()
                && program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| expression_is_fact_observation_free(program, *argument))
        }
        ExpressionNode::Indexed(indexed) => {
            expression_is_fact_observation_free(program, indexed.collection)
                && expression_is_fact_observation_free(program, indexed.index)
        }
        ExpressionNode::Member(member) => {
            expression_is_fact_observation_free(program, member.receiver)
        }
        ExpressionNode::Range(range) => {
            expression_is_fact_observation_free(program, range.start)
                && expression_is_fact_observation_free(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| expression_is_fact_observation_free(program, field.value)),
        ExpressionNode::Unary(unary) => expression_is_fact_observation_free(program, unary.operand),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .all(|value| expression_is_fact_observation_free(program, *value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
    }
}

pub(crate) fn unconditionally_terminates(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return false;
    };
    matches!(
        &machine.termination_plan.checked_summary,
        language_semantics::TerminationGuarantee::Terminates { premises }
            if premises.is_empty()
    )
}

/// Whole-closure purity used by denotational calls.  The selected entry must
/// have no mutable runtime parameter; its machine has no service reach,
/// suspension, or blocking behavior; and every reachable call target is one
/// exact checked machine.
pub(crate) fn has_pure_effect_closure(
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    has_mutable_parameters: bool,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
) -> bool {
    if has_mutable_parameters {
        return false;
    }

    let machine_summaries = operational
        .machines()
        .iter()
        .filter(|summary| summary.symbol == machine_symbol)
        .collect::<Vec<_>>();
    let [machine_summary] = machine_summaries.as_slice() else {
        return false;
    };
    if machine_summary.transitive_may_suspend || machine_summary.transitive_may_block {
        return false;
    }
    if operational
        .states
        .span_or_empty(machine_summary.states)
        .iter()
        .filter(|summary| summary.symbol == state_symbol)
        .count()
        != 1
    {
        return false;
    }

    let reach_summaries = service_reaches
        .machines()
        .iter()
        .filter(|summary| summary.machine == machine_symbol)
        .collect::<Vec<_>>();
    let [reach_summary] = reach_summaries.as_slice() else {
        return false;
    };
    if !service_reaches
        .services(reach_summary.inferred_transitive)
        .is_empty()
    {
        return false;
    }

    let mut pending = vec![machine_symbol];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == current)
            .collect::<Vec<_>>();
        let [summary] = summaries.as_slice() else {
            return false;
        };
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

/// A fact denotes only normal values. Any reachable published crash route
/// makes the call partial in the fact language, even though the route is
/// separately covered for executable use.
pub(crate) fn has_no_crash_routes(
    program: &TypedTrees,
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
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == current)
        else {
            return false;
        };
        let crashes = program
            .machine_contracts(machine)
            .iter()
            .chain(
                program
                    .machine_states(machine)
                    .iter()
                    .flat_map(|state| program.state_contracts(state)),
            )
            .any(|contract| {
                matches!(
                    contract.kind,
                    typed_trees::signature::SignatureContractKind::Crashes { .. }
                )
            });
        if crashes {
            return false;
        }
        let summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == current)
            .collect::<Vec<_>>();
        let [summary] = summaries.as_slice() else {
            return false;
        };
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
