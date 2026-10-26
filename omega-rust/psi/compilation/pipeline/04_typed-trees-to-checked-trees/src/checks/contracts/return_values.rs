//! Exact normal-return occurrences shared by exit proof consumers.

use checked_trees::{CheckFacts, FlowCallFact, FlowExitFact, FlowStateFact};
use typed_trees::{
    expression::ExpressionHandle,
    machine::Machine,
    statement::{StatementNode, TransitionExit, TransitionTargetNode},
};

/// A completed opaque activation claim cannot cross its owning activation's
/// return boundary. Check the actual returning occurrence, not its signature:
/// crashing bodies and claim-free Rejected attempts do not return a claim.
/// Aggregate/conditional custody is decided on the final executable CFG.
pub(super) fn check_activation_return(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    exit: &FlowExitFact,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let Some(machine) = program.machine(exit.machine_symbol) else {
        return;
    };
    let Some(state) = validation::find_state_for_exit(program, exit) else {
        return;
    };
    let returned = exit_return_expression(program, exit);
    let reference = if returned.is_valid() {
        validation::expression_result_type_reference(program, machine, state, returned)
    } else {
        exit_return_call_in_state(program, facts, state_flow, exit)
            .and_then(|call| validation::call_target_state(program, call.target_symbol))
            .map(|target| target.return_type)
    };
    let Some(symbol) = reference.and_then(|reference| {
        program
            .type_reference_table
            .type_reference(reference)
            .as_named_symbol()
    }) else {
        return;
    };
    if validation::activation_claim_byte_carrier(program, symbol).is_none() {
        return;
    }
    let mut diagnostic = diagnostics::Diagnostic::error(format!(
        "activation claim cannot escape its owning activation through return from {} state {} at statement {}",
        program.symbols.display_path(exit.machine_symbol, "::"),
        program.symbols.display_path(exit.state_symbol, "::"),
        exit.statement_index,
    ));
    if returned.is_valid() {
        let span = program.expression_table.source_span(returned);
        if span != source::SourceSpan::default() {
            diagnostic = diagnostic.with_source_span(span);
        }
    }
    diagnostics.push(diagnostic);
}

/// The completed operand belongs to this exact outgoing occurrence. An exit's
/// source position alone does not establish that its expression has finished:
/// same-statement call inputs still use their earlier evaluation frontier.
pub(in crate::checks::contracts) fn completed_return_operand<'facts>(
    program: &dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    exit: &FlowExitFact,
) -> Option<&'facts checked_trees::FlowOperandFact> {
    if !facts.flow.control.operands.is_valid(exit.returned_value) {
        return None;
    }
    let operand = facts.flow.control.operands.get(exit.returned_value);
    let returned = exit_return_expression(program, exit);
    if !returned.is_valid()
        || operand.expression != returned
        || operand.state_symbol != exit.state_symbol
        || operand.statement_index != exit.statement_index
    {
        return None;
    }
    let mut outgoing = facts.flow.control.outgoing.iter().filter(|(_, outgoing)| {
        outgoing.machine_symbol == exit.machine_symbol
            && outgoing.state_symbol == exit.state_symbol
            && outgoing.statement_index == exit.statement_index
            && outgoing.transition_target == exit.transition_target
            && outgoing.returned_value == exit.returned_value
    });
    outgoing.next()?;
    outgoing.next().is_none().then_some(operand)
}

/// Rejoin a returned invocation to its exact retained call occurrence, whether
/// the return spells a value call or names an external tail target.
pub(super) fn exit_return_call<'facts>(
    program: &dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    exit: &FlowExitFact,
) -> Option<&'facts FlowCallFact> {
    let returned = exit_return_expression(program, exit);
    let calls = facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == exit.machine_symbol && state.state_symbol == exit.state_symbol
        })
        .flat_map(|(_, state)| facts.flow.control.calls.span_or_empty(state.calls));
    matching_exit_return_call(program, exit, returned, calls)
}

pub(super) fn exit_return_call_in_state<'facts>(
    program: &dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    state_flow: &FlowStateFact,
    exit: &FlowExitFact,
) -> Option<&'facts FlowCallFact> {
    let returned = exit_return_expression(program, exit);
    matching_exit_return_call(
        program,
        exit,
        returned,
        facts.flow.control.calls.span_or_empty(state_flow.calls),
    )
}

fn matching_exit_return_call<'facts>(
    program: &dyn ::validation::CheckingInput,
    exit: &FlowExitFact,
    returned: ExpressionHandle,
    calls: impl IntoIterator<Item = &'facts FlowCallFact>,
) -> Option<&'facts FlowCallFact> {
    let mut matching = calls
        .into_iter()
        .filter(|call| call.statement_index == exit.statement_index)
        .filter(|call| {
            let Some(site) = validation::find_call_site(
                program,
                exit.machine_symbol,
                exit.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                return false;
            };
            match site {
                validation::CallSite::Expression {
                    expression,
                    call: selected,
                } => {
                    returned.is_valid()
                        && expression == returned
                        && call.authored_expression == returned
                        && call.target_symbol == selected.target_symbol
                }
                validation::CallSite::TransitionNamed { path: selected, .. }
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
                        && call.target_symbol
                            == validation::call_target_state(program, path.symbol)
                                .map_or(path.symbol, |state| state.symbol)
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
    program: &dyn ::validation::CheckingInput,
    exit: &FlowExitFact,
) -> ExpressionHandle {
    let Some(state) = validation::find_state_for_exit(program, exit) else {
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
    program: &dyn ::validation::CheckingInput,
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
