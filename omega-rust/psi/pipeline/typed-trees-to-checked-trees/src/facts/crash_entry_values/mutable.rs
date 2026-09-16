//! Whether a mutable binding's current storage still holds the value its
//! binding position established.
//!
//! A `mut` parameter is bound by the invocation (entry state) or by each
//! named transition edge into its state; a `let mut` local is bound by its
//! initializer. A read sees that bound snapshot only while no intervening
//! statement can overwrite the storage or lend it exclusive access. The scan
//! is deliberately syntactic and conservative: anything it cannot rule out
//! dirties the snapshot, so provenance stays unknown rather than treating a
//! later write as the saved actual.

use crate::lookup::expression_root_symbol;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{
    StatementNode, TableTransition, TransitionGuardNode, TransitionTargetNode,
};

/// Ordinals of `state`'s transition statements that re-enter it through
/// `-> self`, forwarding the current parameter values as a fresh arrival.
/// For a mutable parameter that arrival binds whatever its storage holds at
/// the edge, so each ordinal needs its own pristine prefix.
pub(super) fn self_target_ordinals(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Vec<usize> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            [transition.target, transition.continuation]
                .into_iter()
                .filter(|target| target.is_valid())
                .any(|target| {
                    matches!(
                        program.statement_table.transition_target(target),
                        TransitionTargetNode::SelfTarget
                    )
                })
                .then_some(ordinal)
        })
        .collect()
}

/// No statement in `state[start..end]` may overwrite `symbol`'s storage or
/// lend it exclusive access. A malformed window cannot be cleared.
pub(super) fn storage_holds_bound_value(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    start: usize,
    end: usize,
    symbol: SymbolHandle,
) -> bool {
    let Some(window) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(start..end)
    else {
        return false;
    };
    !window
        .iter()
        .any(|statement| statement_may_overwrite(program, machine_symbol, statement, symbol))
}

fn statement_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    symbol: SymbolHandle,
) -> bool {
    match statement {
        StatementNode::Assignment(assignment) => {
            expression_root_symbol(assignment.target, &program.expression_table, machine_symbol)
                == Some(symbol)
                || expression_may_overwrite(program, machine_symbol, assignment.target, symbol)
                || expression_may_overwrite(program, machine_symbol, assignment.value, symbol)
        }
        StatementNode::RootBinding(binding) => {
            expression_root_symbol(binding.receiver, &program.expression_table, machine_symbol)
                == Some(symbol)
                || expression_may_overwrite(program, machine_symbol, binding.receiver, symbol)
                || (binding.implementation_operand.is_valid()
                    && expression_may_overwrite(
                        program,
                        machine_symbol,
                        binding.implementation_operand,
                        symbol,
                    ))
        }
        StatementNode::AssemblyFact(fact) => {
            expression_may_overwrite(program, machine_symbol, fact.expression, symbol)
        }
        StatementNode::Expression(expression) => {
            expression_may_overwrite(program, machine_symbol, *expression, symbol)
        }
        StatementNode::LocalData(local) => {
            expression_may_overwrite(program, machine_symbol, local.initial_value, symbol)
        }
        StatementNode::Call(call) => {
            (call.receiver_root_symbol == symbol
                && receiver_call_writes(program, call.target_symbol))
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| {
                        expression_may_overwrite(program, machine_symbol, *argument, symbol)
                    })
        }
        StatementNode::Transition(transition) => {
            transition_may_overwrite(program, machine_symbol, transition, symbol)
        }
    }
}

/// Guard, named-target arguments, and value targets of one transition. A
/// nested exclusive borrow or receiver-mutating call inside any of them can
/// reach `symbol`'s storage before the edge is taken.
fn transition_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    transition: &TableTransition,
    symbol: SymbolHandle,
) -> bool {
    if let TransitionGuardNode::When(guard) = &transition.guard
        && expression_may_overwrite(program, machine_symbol, *guard, symbol)
    {
        return true;
    }
    [transition.target, transition.continuation]
        .into_iter()
        .filter(|target| target.is_valid())
        .any(
            |target| match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { arguments, .. } => program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| {
                        expression_may_overwrite(program, machine_symbol, *argument, symbol)
                    }),
                TransitionTargetNode::Value(value) => {
                    expression_may_overwrite(program, machine_symbol, *value, symbol)
                }
                TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
            },
        )
}

/// `symbol`'s storage is reached by an exclusive borrow or by a call whose
/// callee may write its `self` receiver. Writes to a different binding are
/// not followed; their own `&mut symbol` creation is what dirties `symbol`.
fn expression_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    root: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            // An unknown subtree cannot be cleared of reaching the storage.
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => {
                if borrow.access.is_exclusive()
                    && expression_root_symbol(
                        borrow.target,
                        &program.expression_table,
                        machine_symbol,
                    ) == Some(symbol)
                {
                    return true;
                }
                pending.push(borrow.target);
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid()
                    && expression_root_symbol(
                        call.receiver,
                        &program.expression_table,
                        machine_symbol,
                    ) == Some(symbol)
                    && receiver_call_writes(program, call.target_symbol)
                {
                    return true;
                }
                // A receiverless call leaves this handle invalid; only
                // present children are scanned.
                if call.receiver.is_valid() {
                    pending.push(call.receiver);
                }
                pending.extend(
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::Atomic(atomic) => {
                if expression_root_symbol(atomic.value, &program.expression_table, machine_symbol)
                    == Some(symbol)
                {
                    return true;
                }
                pending.push(atomic.value);
                if atomic.result.is_valid() {
                    pending.push(atomic.result);
                }
            }
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Indexed(indexed) => {
                pending.push(indexed.collection);
                pending.push(indexed.index);
            }
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Range(range) => {
                pending.push(range.start);
                pending.push(range.end);
            }
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend(
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    pending.push(field.value);
                }
            }
            _ => {}
        }
    }
    false
}

/// A call whose receiver is rooted at the tracked symbol writes that storage
/// when the callee's `self` parameter is mutable. An unresolvable or
/// self-less target cannot be cleared of writing.
fn receiver_call_writes(program: &TypedTrees, target_symbol: SymbolHandle) -> bool {
    match crate::semantic_calls::call_target_parameters(program, target_symbol) {
        Some(parameters) => parameters
            .iter()
            .find(|parameter| parameter.is_self)
            .is_none_or(|parameter| parameter.is_mutable),
        None => true,
    }
}
