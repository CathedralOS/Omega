//! Exact normal-return occurrences shared by exit proof consumers.

use checked_trees::{CheckFacts, FlowCallFact, FlowExitFact};
use typed_trees::{
    TypedTrees,
    expression::ExpressionHandle,
    machine::Machine,
    statement::{StatementNode, TransitionExit, TransitionTargetNode},
};

/// Rejoin a returned invocation to its exact retained call occurrence, whether
/// the return spells a value call or names an external tail target.
pub(super) fn exit_return_call<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    exit: &FlowExitFact,
) -> Option<&'facts FlowCallFact> {
    let returned = exit_return_expression(program, exit);
    let mut matching = facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == exit.machine_symbol && state.state_symbol == exit.state_symbol
        })
        .flat_map(|(_, state)| facts.flow.control.calls.span_or_empty(state.calls))
        .filter(|call| call.statement_index == exit.statement_index)
        .filter(|call| {
            let Some(site) = crate::semantic::calls::find_call_site(
                program,
                exit.machine_symbol,
                exit.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                return false;
            };
            match site {
                crate::semantic::calls::CallSite::Expression {
                    expression,
                    call: selected,
                } => {
                    returned.is_valid()
                        && expression == returned
                        && call.authored_expression == returned
                        && call.target_symbol == selected.target_symbol
                }
                crate::semantic::calls::CallSite::TransitionNamed { path: selected, .. }
                    if exit.transition_target.is_valid() =>
                {
                    let TransitionTargetNode::Named { path, .. } = program
                        .statement_table
                        .transition_target(exit.transition_target)
                    else {
                        return false;
                    };
                    // Both borrows name the same immutable table occurrence;
                    // equal targets in sibling arms are not interchangeable.
                    std::ptr::eq(path, selected)
                        && call.target_symbol == path.symbol
                        && crate::proof::contract_target_from_state_symbol(program, path.symbol)
                            .is_some_and(|(owner, _)| owner != exit.machine_symbol)
                }
                _ => false,
            }
        });
    let call = matching.next()?;
    matching.next().is_none().then_some(call)
}

pub(super) fn exit_return_expression(
    program: &TypedTrees,
    exit: &FlowExitFact,
) -> ExpressionHandle {
    let Some(state) = crate::semantic::calls::find_state_in_machine(
        program,
        exit.machine_symbol,
        exit.state_symbol,
    ) else {
        return Default::default();
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    match statements.get(exit.statement_index) {
        Some(StatementNode::Expression(value))
            if !exit.transition_target.is_valid()
                && exit.statement_index.checked_add(1) == Some(statements.len()) =>
        {
            *value
        }
        Some(StatementNode::Transition(transition))
            if exit.transition_target.is_valid()
                && [transition.target, transition.continuation]
                    .contains(&exit.transition_target)
                && transition.exit == TransitionExit::Ordinary =>
        {
            match program
                .statement_table
                .transition_target(exit.transition_target)
            {
                TransitionTargetNode::Value(value) => *value,
                _ => Default::default(),
            }
        }
        _ => Default::default(),
    }
}

pub(super) fn is_result_reference(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    // The current typed contract surface retains the synthetic result name,
    // not a result binder handle. An authored entry parameter takes precedence.
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    language_core::signature_reserves_result(
        program
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.name.as_str()),
    ) && program.expression_table.names_reserved_result(expression)
}
