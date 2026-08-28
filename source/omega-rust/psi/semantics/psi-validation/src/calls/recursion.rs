//! Runtime recursive-call position validation.
//!
//! This module owns runtime tail-position checks. Proof-machine structural and
//! cited decrease validation lives in the focused child module; the parent call
//! validator controls traversal and diagnostic ordering.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

mod proof_machines;

pub(crate) use proof_machines::validate_proof_machine_recursion;

/// Measured recursion MR3 (2026-07-18 ruling): runtime recursion is
/// TAIL-ONLY, and the only tail position is the transition ARM TARGET
/// (`-> self.f(..)` on a measured machine, resolved onto the loop-back
/// edge). This walk names every OTHER self-recursive call spelling with
/// why it is not tail:
/// - embedded in a larger expression (`3 * self.f(n - 1)`, an argument, a
///   guard, an initializer): the frame must survive the call to finish the
///   surrounding computation -- non-tail, CUT by the amendment (depth
///   lives in explicit storage the author sizes);
/// - a state's bare terminal expression (`{ self.sum(n - 1, acc) }`): tail
///   in shape, but its loop-back rewrite is the MR2 rung -- refused with
///   that pointer until the lowering lands.
pub(crate) fn validate_self_recursive_call_positions(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str());
    match statement {
        StatementNode::AssemblyFact(_) => {}
        // The statement-position fence in `validate_call_node` owns
        // StatementNode::Call; transition ARM TARGETS are the legal tail
        // spelling (planned by the state graph). Everything else that can
        // hold an expression walks below.
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                reject_embedded_self_calls(program, machine, entry_name, *argument, diagnostics);
            }
        }
        StatementNode::Assignment(assignment) => {
            reject_embedded_self_calls(program, machine, entry_name, assignment.value, diagnostics);
        }
        StatementNode::LocalData(local_data) => {
            reject_embedded_self_calls(
                program,
                machine,
                entry_name,
                local_data.initial_value,
                diagnostics,
            );
        }
        StatementNode::Expression(expression) => {
            // A bare terminal self-call is TAIL in shape; the whole-expression
            // case gets the MR2 pointer, anything nested is non-tail.
            // A terminal self-call surviving to validation means the machine
            // is UNMEASURED: the parser rewrites measured machines' terminal
            // tail calls onto the loop-back edge (MR2).
            if let Some(call_display) = whole_expression_self_call(program, entry_name, *expression)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{call_display}` in terminal position is TAIL self-recursion on an \
                     UNMEASURED machine. Recursive call spellings are legal only when \
                     measured: declare `terminates by ...;` and the terminal \
                     call rewrites onto the loop-back edge; unmeasured repetition spells \
                     as the bare loop `-> {entry_name}(..)` (constant stack, may diverge).",
                )));
                return;
            }
            reject_embedded_self_calls(program, machine, entry_name, *expression, diagnostics);
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                reject_embedded_self_calls(program, machine, entry_name, guard, diagnostics);
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    // Arm-target arguments are evaluated AT the jump; a
                    // self-call inside one still needs its own frame first.
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.statement_table.expression_handles(*arguments) {
                            reject_embedded_self_calls(
                                program,
                                machine,
                                entry_name,
                                *argument,
                                diagnostics,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        reject_embedded_self_calls(
                            program,
                            machine,
                            entry_name,
                            *expression,
                            diagnostics,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    let _ = state;
}

/// The rendered call when `expression` IS a self-call to the machine's own
/// entry (the terminal-position tail shape); None otherwise.
fn whole_expression_self_call(
    program: &TypedTrees,
    entry_name: &str,
    expression: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    is_self_entry_call(program, entry_name, call).then(|| format!("self.{entry_name}(..)"))
}

fn is_self_entry_call(program: &TypedTrees, entry_name: &str, call: &TableCallExpression) -> bool {
    if call.target.as_str() != entry_name {
        return false;
    }
    !call.receiver.is_valid()
        || matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path)
                if matches!(
                    program.expression_table.name_path_members(path.members),
                    [only] if only.as_str() == "self"
                )
        )
}

/// Reject every self-entry call in this expression tree: any hit here is
/// embedded in a larger computation, so the frame outlives the call.
fn reject_embedded_self_calls(
    program: &TypedTrees,
    machine: &Machine,
    entry_name: &str,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    let recurse = |handle: ExpressionHandle, diagnostics: &mut Vec<Diagnostic>| {
        reject_embedded_self_calls(program, machine, entry_name, handle, diagnostics);
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value, diagnostics),
        ExpressionNode::Call(call) => {
            if is_self_entry_call(program, entry_name, call) {
                diagnostics.push(Diagnostic::error(format!(
                    "`self.{entry_name}(..)` here is NON-TAIL self-recursion: the \
                     result feeds the surrounding computation, so the frame must \
                     survive the call. Runtime recursion is TAIL-ONLY (measured \
                     recursion amendment) -- recursion depth lives in explicit \
                     storage you size. Restructure so the recursive step is the \
                     transition arm `-> self.{entry_name}(..)` on a measured \
                     machine, or iterate with an explicit stack.",
                )));
            }
            recurse(call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left, diagnostics);
            recurse(binary.right, diagnostics);
        }
        ExpressionNode::Cast(cast) => recurse(cast.value, diagnostics),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection, diagnostics);
            recurse(indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => recurse(member.receiver, diagnostics),
        ExpressionNode::Borrow(inner) => recurse(inner.target, diagnostics),
        ExpressionNode::Range(range) => {
            recurse(range.start, diagnostics);
            recurse(range.end, diagnostics);
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand, diagnostics),
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                recurse(*item, diagnostics);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                recurse(field.value, diagnostics);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
