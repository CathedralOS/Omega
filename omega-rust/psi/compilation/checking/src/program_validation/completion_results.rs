//! Unit completion owes the same declared result on every ordinary exit.
//!
//! Explicit `{}` arms and falling off a checked body both deliver Unit. The
//! old empty-body check missed writes followed by fallthrough, explicit Unit
//! arms and generic bodies. Inspect the admitted instruction order instead of
//! recognizing those source arrangements. Value-result checks retain their
//! expression/type owners; a crash or an authenticated nonreturning invocation
//! has no normal result to admit. Bodyless requirements declare a contract,
//! rather than an executable empty implementation.

use crate::machine_calls::calls;
use diagnostics::Diagnostic;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use typed_trees::types::TypeReferenceNode;

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !machine.supplies_checked_body() || is_unit(program, state) {
        return;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let Ok(Some(order)) = program
        .statement_table
        .finite_execution_order(state.statement_nodes)
    else {
        // Control admission owns invalid and non-finite assembly graphs. No
        // finite end is established here for a genuinely cyclic body.
        return;
    };
    for (position, statement_index) in order.iter().copied().enumerate() {
        match &statements[statement_index] {
            StatementNode::Transition(transition) => {
                if transition.exit == TransitionExit::Ordinary {
                    for target in transition.targets() {
                        if matches!(
                            program.statement_table.transition_target(target),
                            TransitionTargetNode::Terminal
                        ) {
                            report(program, machine, state, "a terminal arm", diagnostics);
                        }
                    }
                }
                if transition.guard == TransitionGuardNode::Always
                    || transition.continuation.is_valid()
                {
                    return;
                }
            }
            StatementNode::Expression(expression) => {
                if position + 1 == order.len()
                    || calls::expression_normal_completion_possible(program, *expression)
                        == Some(false)
                {
                    return;
                }
            }
            StatementNode::Call(call)
                if calls::target_normal_completion_possible(program, call.target_symbol)
                    == Some(false) =>
            {
                return;
            }
            StatementNode::LocalData(local)
                if local.initial_value.is_valid()
                    && calls::expression_normal_completion_possible(program, local.initial_value)
                        == Some(false) =>
            {
                return;
            }
            _ => {}
        }
    }
    if statements.is_empty() {
        // Retain the existing public diagnostic for the empty-body case.
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` declares a return type but its body is empty -- return a value or drop the `-> T`",
            machine.name, state.name,
        )));
    } else {
        report(
            program,
            machine,
            state,
            "implicit body completion",
            diagnostics,
        );
    }
}

fn is_unit(program: &dyn crate::CheckingInput, state: &State) -> bool {
    let mut reference = state.return_type;
    if !reference.is_valid() {
        return true;
    }
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Unit => return true,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return false,
        }
    }
}

fn report(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    completion: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` {completion} supplies Unit, but the invocation returns `{}`; every normal completion must supply its declared result",
        machine.name, state.name, program.display_type_reference_with_constraints(state.return_type),
    )));
}
