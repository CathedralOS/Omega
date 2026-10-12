//! Runtime recursive-call position validation.
//!
//! This module owns runtime tail-position checks. Proof-machine structural and
//! cited decrease validation lives in the focused child module; the parent call
//! validator controls traversal and diagnostic ordering.

use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

mod proof_machines;
pub use proof_machines::ValidatedSelfCallDescent;

pub(crate) use proof_machines::proof_call_has_structural_descent;
pub(crate) use proof_machines::validate_proof_machine_recursion;

/// Runtime recursion must not leave a pending caller computation. Exact
/// whole calls in terminal expression/return positions retain their original
/// source admission. Calls embedded in arithmetic, guards or initializers are
/// not tail. Exact current-instance reuse is a later checked control relation;
/// source tail position alone grants neither that relation nor a decrease.
pub(crate) fn validate_self_recursive_call_positions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            for expression in [binding.receiver, binding.implementation_operand] {
                if expression.is_valid() {
                    reject_embedded_self_calls(program, machine, expression, diagnostics);
                }
            }
        }
        StatementNode::AssemblyFact(_) => {}
        // The statement-position fence in `validate_call_node` owns
        // StatementNode::Call; transition ARM TARGETS are the legal tail
        // spelling (planned by the state graph). Everything else that can
        // hold an expression walks below.
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                reject_embedded_self_calls(program, machine, *argument, diagnostics);
            }
        }
        StatementNode::Assignment(assignment) => {
            reject_embedded_self_calls(program, machine, assignment.value, diagnostics);
        }
        StatementNode::LocalData(local_data) => {
            reject_embedded_self_calls(program, machine, local_data.initial_value, diagnostics);
        }
        StatementNode::Expression(expression) => {
            if let Some(call) = source_tail_call(program, state, statement_index, *expression) {
                reject_tail_operand_self_calls(program, machine, call, diagnostics);
                return;
            }
            reject_embedded_self_calls(program, machine, *expression, diagnostics);
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                reject_embedded_self_calls(program, machine, guard, diagnostics);
            }
            for target_handle in transition.targets() {
                match program.statement_table.transition_target(target_handle) {
                    // Arm-target arguments are evaluated AT the jump; a
                    // self-call inside one still needs its own frame first.
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.statement_table.expression_handles(*arguments) {
                            reject_embedded_self_calls(program, machine, *argument, diagnostics);
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        if let Some(call) =
                            source_tail_call(program, state, statement_index, *expression)
                        {
                            reject_tail_operand_self_calls(program, machine, call, diagnostics);
                        } else {
                            reject_embedded_self_calls(program, machine, *expression, diagnostics);
                        }
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    let _ = state;
}

/// The rendered call when `expression` IS a self-call to the machine's own
/// entry (the terminal-position tail shape); None otherwise.
fn is_current_entry_call(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    call: &TableCallExpression,
) -> bool {
    // Qualification and leaf spelling do not define recursion. The exact
    // selected current entry does, including declaration-qualified calls.
    call.target_symbol.is_valid()
        && (call.target_symbol == machine.symbol
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| call.target_symbol == entry.symbol))
}

/// Reject every self-entry call in this expression tree: any hit here is
/// embedded in a larger computation, so the frame outlives the call.
fn reject_embedded_self_calls(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    let recurse = |handle: ExpressionHandle, diagnostics: &mut Vec<Diagnostic>| {
        reject_embedded_self_calls(program, machine, handle, diagnostics);
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value, diagnostics),
        ExpressionNode::Call(call) => {
            if is_current_entry_call(program, machine, call) {
                // A transition arm names the entry by its leaf; the
                // qualification that selects the machine is not respelled
                // there.
                let path = machine.name.as_str();
                let leaf = path.rsplit("::").next().unwrap_or(path);
                diagnostics.push(Diagnostic::error(format!(
                    "`{path}(..)` here is NON-TAIL self-recursion: the \
                     result feeds the surrounding computation, so the frame must \
                     survive the call. Runtime recursion is TAIL-ONLY (measured \
                     recursion amendment) -- recursion depth lives in explicit \
                     storage you size. Restructure so the recursive step is the \
                     transition arm `-> {leaf}(..)` on a measured \
                     machine, or iterate with an explicit stack.",
                )));
            }
            recurse(call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, diagnostics);
            }
        }
        other => crate::expression_traversal::children(program, other, |child| {
            recurse(child, diagnostics);
        }),
    }
}

/// Source tail position carries no pending result computation. Ordinary
/// application, receiver, effects, regions and ranking admission still judge
/// this same occurrence. An abstract generic declaration has no concrete
/// instance tuple to equate with its recursive actuals; physical entry reuse
/// therefore belongs to the separately fenced query below.
pub fn source_tail_call<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<&'program TableCallExpression> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let statement = statements.get(statement_index)?;
    let terminal = match statement {
        StatementNode::Expression(selected) => {
            *selected == expression && statement_index + 1 == statements.len()
        }
        StatementNode::Transition(transition) => {
            transition.exit == typed_trees::statement::TransitionExit::Ordinary
                && transition.targets().any(|target| {
                    matches!(program.statement_table.transition_target(target),
                        TransitionTargetNode::Value(selected) if *selected == expression)
                })
        }
        _ => false,
    };
    if !terminal {
        return None;
    }
    program.expression_table.call_node(expression)
}

/// Exact whole terminal call that can reuse the current entry activation.
/// This query classifies source shape and selected instance identity only. The
/// ordinary call retains its static/public contract, effects and arguments;
/// checked Flow subsequently owns jump captures and source-scope loan closure.
pub fn current_entry_tail_call<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<&'program TableCallExpression> {
    let call = source_tail_call(program, state, statement_index, expression)?;
    if !is_current_entry_call(program, machine, call) {
        return None;
    }
    if call.receiver.is_valid()
        && !matches!(program.expression_table.expression(call.receiver),
            ExpressionNode::Name(receiver)
                if machine.attached_data_symbol.is_valid()
                    && receiver.symbol == machine.attached_data_symbol)
    {
        return None;
    }
    let source = program.expression_table.source_call(call)?;
    if source.receiver.is_valid() {
        let mut instances = program
            .machine_specializations
            .iter()
            .filter(|instance| instance.instance == machine.symbol);
        let source_machine = if let Some(instance) = instances.next() {
            if instances.next().is_some() {
                return None;
            }
            program.machine_by_symbol(instance.template)?
        } else {
            machine
        };
        if !matches!(program.expression_table.expression(source.receiver),
            ExpressionNode::Name(receiver) if source_machine.attached_data_symbol.is_valid()
                && receiver.symbol == source_machine.attached_data_symbol)
        {
            return None;
        }
    }
    if !source.authored_machine_application.is_valid()
        && !is_current_entry_call(program, machine, source)
    {
        return None;
    }
    if !typed_trees::static_applications::machine_application_selects_current_instance(
        program.typed_program(),
        machine,
        source.authored_machine_application,
    ) {
        return None;
    }
    Some(call)
}

fn reject_tail_operand_self_calls(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    reject_embedded_self_calls(program, machine, call.receiver, diagnostics);
    for argument in program.expression_table.expression_handles(call.arguments) {
        reject_embedded_self_calls(program, machine, *argument, diagnostics);
    }
}
