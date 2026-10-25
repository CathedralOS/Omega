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

/// Runtime recursion must not leave a pending caller computation. A whole
/// terminal call forwards completion; a call nested in its receiver, operands,
/// guards or constructors does not. Position is independent of whether a
/// generic application has normalized to a named state edge. The component's
/// ranking and native realization separately establish a constant-stack route.
pub(crate) fn validate_self_recursive_call_positions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
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
            validate_terminal_value(program, machine, *expression, diagnostics);
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
                        validate_terminal_value(program, machine, *expression, diagnostics);
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    let _ = state;
}

/// The outer invocation completes the machine. Its own operands still finish
/// first, so recursively calling this machine inside any operand remains non-tail.
fn validate_terminal_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(call) = program.expression_table.call_node(expression) {
        reject_embedded_self_calls(program, machine, call.receiver, diagnostics);
        for argument in program.expression_table.expression_handles(call.arguments) {
            reject_embedded_self_calls(program, machine, *argument, diagnostics);
        }
    } else {
        reject_embedded_self_calls(program, machine, expression, diagnostics);
    }
}

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
