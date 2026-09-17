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
//!
//! Field versions refine the same rule below the binding root: a read of
//! `rec.value` keeps its bound snapshot across writes, exclusive borrows, and
//! receiver-mutating calls confined to sibling fields such as `rec.other`.
//! The rooted place path of every write is compared against the read path;
//! disjoint sibling projections do not interfere, while an indexed element, a
//! case payload, or any projection the scan cannot separate stays opaque and
//! reaches everything at or below it.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
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

/// One projection step below a binding's root. `Opaque` marks a position the
/// scan cannot separate — an indexed element, a case-qualified payload, or an
/// unresolvable member — which interferes with every read at or below it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PlaceSegment {
    Field(SymbolHandle),
    Opaque,
}

/// A write to `write` reaches a read of `read` only when the two paths cannot
/// be separated: equal segments until one path ends (either covers the
/// other), or an opaque step. Distinct sibling fields never interfere.
fn paths_interfere(write: &[PlaceSegment], read: &[PlaceSegment]) -> bool {
    for (write, read) in write.iter().zip(read.iter()) {
        match (write, read) {
            (PlaceSegment::Field(write), PlaceSegment::Field(read)) => {
                if write != read {
                    return false;
                }
            }
            _ => return true,
        }
    }
    true
}

/// The binding root plus projection steps of a place expression, mirroring
/// `expression_root_symbol`'s rooting — `self.<field>` roots at the machine
/// field — while retaining the segments a whole-symbol scan discards. A
/// non-place expression has no root.
fn rooted_place_path(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<PlaceSegment>)> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            rooted_place_path(program, machine_symbol, indexed.collection).map(
                |(root, mut path)| {
                    path.push(PlaceSegment::Opaque);
                    (root, path)
                },
            )
        }
        ExpressionNode::Borrow(borrow) => rooted_place_path(program, machine_symbol, borrow.target),
        ExpressionNode::Member(member) => {
            if let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
                && path.members.count() == 1
                && path.symbol.is_valid()
                && path.symbol == machine_symbol
            {
                return member
                    .member_symbol
                    .is_valid()
                    .then_some((member.member_symbol, Vec::new()));
            }
            rooted_place_path(program, machine_symbol, member.receiver).map(|(root, mut path)| {
                path.push(
                    if member.case_variant.is_none() && member.member_symbol.is_valid() {
                        PlaceSegment::Field(member.member_symbol)
                    } else {
                        PlaceSegment::Opaque
                    },
                );
                (root, path)
            })
        }
        ExpressionNode::Name(path) => {
            crate::lookup::first_valid_name_path_symbol(path, &program.expression_table)
                .map(|root| (root, Vec::new()))
        }
        _ => None,
    }
}

/// The written or exclusively borrowed place `expression` reaches `symbol`'s
/// storage at a path the read cannot be separated from.
fn place_interferes(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    rooted_place_path(program, machine_symbol, expression)
        .is_some_and(|(root, write_path)| root == symbol && paths_interfere(&write_path, read_path))
}

/// Whether `statement` may overwrite the canonical `place` at all — a write,
/// an exclusive borrow, or a receiver-mutating call the scan cannot separate
/// from the read. Non-symbol roots cannot be reasoned about and stay
/// conservative. A caller-side value proof uses this to refuse reads whose
/// call-entry context postdates a sibling operand's effects.
pub(crate) fn statement_may_overwrite_place(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return true;
    };
    let read_path: Vec<PlaceSegment> = place
        .segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => PlaceSegment::Field(*symbol),
            _ => PlaceSegment::Opaque,
        })
        .collect();
    statement_may_overwrite(program, machine_symbol, statement, symbol, &read_path)
}

/// No statement in `state[start..end]` may overwrite `symbol`'s storage at
/// `read_path` or lend that projection exclusive access. An empty read path
/// is the whole binding: every write rooted at `symbol` interferes. A
/// malformed window cannot be cleared.
pub(super) fn storage_holds_bound_value(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    start: usize,
    end: usize,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    let Some(window) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(start..end)
    else {
        return false;
    };
    !window.iter().any(|statement| {
        statement_may_overwrite(program, machine_symbol, statement, symbol, read_path)
    })
}

fn statement_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    match statement {
        StatementNode::Assignment(assignment) => {
            place_interferes(
                program,
                machine_symbol,
                assignment.target,
                symbol,
                read_path,
            ) || expression_may_overwrite(
                program,
                machine_symbol,
                assignment.target,
                symbol,
                read_path,
            ) || expression_may_overwrite(
                program,
                machine_symbol,
                assignment.value,
                symbol,
                read_path,
            )
        }
        StatementNode::RootBinding(binding) => {
            place_interferes(program, machine_symbol, binding.receiver, symbol, read_path)
                || expression_may_overwrite(
                    program,
                    machine_symbol,
                    binding.receiver,
                    symbol,
                    read_path,
                )
                || (binding.implementation_operand.is_valid()
                    && expression_may_overwrite(
                        program,
                        machine_symbol,
                        binding.implementation_operand,
                        symbol,
                        read_path,
                    ))
        }
        StatementNode::AssemblyFact(fact) => {
            expression_may_overwrite(program, machine_symbol, fact.expression, symbol, read_path)
        }
        StatementNode::Expression(expression) => {
            expression_may_overwrite(program, machine_symbol, *expression, symbol, read_path)
        }
        StatementNode::LocalData(local) => expression_may_overwrite(
            program,
            machine_symbol,
            local.initial_value,
            symbol,
            read_path,
        ),
        StatementNode::Call(call) => {
            (call.receiver_root_symbol == symbol
                && receiver_call_writes(program, call.target_symbol)
                && call_receiver_path_interferes(program, call, read_path))
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| {
                        expression_may_overwrite(
                            program,
                            machine_symbol,
                            *argument,
                            symbol,
                            read_path,
                        )
                    })
        }
        StatementNode::Transition(transition) => {
            transition_may_overwrite(program, machine_symbol, transition, symbol, read_path)
        }
    }
}

/// A statement call's receiver place covers the read path. The receiver is a
/// name path, not an expression: `x` covers the whole binding, `x.<field>`
/// confines the write to the resolved leaf member, and a deeper or
/// unresolvable projection cannot be separated below its root.
fn call_receiver_path_interferes(
    program: &TypedTrees,
    call: &TableCall,
    read_path: &[PlaceSegment],
) -> bool {
    let write_path: Vec<PlaceSegment> = match program
        .statement_table
        .name_path_members(call.receiver)
        .len()
    {
        0 | 1 => Vec::new(),
        2 if call.receiver_symbol.is_valid() => {
            vec![PlaceSegment::Field(call.receiver_symbol)]
        }
        _ => vec![PlaceSegment::Opaque],
    };
    paths_interfere(&write_path, read_path)
}

/// Guard, named-target arguments, and value targets of one transition. A
/// nested exclusive borrow or receiver-mutating call inside any of them can
/// reach `symbol`'s storage before the edge is taken.
fn transition_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    transition: &TableTransition,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    if let TransitionGuardNode::When(guard) = &transition.guard
        && expression_may_overwrite(program, machine_symbol, *guard, symbol, read_path)
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
                        expression_may_overwrite(
                            program,
                            machine_symbol,
                            *argument,
                            symbol,
                            read_path,
                        )
                    }),
                TransitionTargetNode::Value(value) => {
                    expression_may_overwrite(program, machine_symbol, *value, symbol, read_path)
                }
                TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
            },
        )
}

/// `symbol`'s storage at `read_path` is reached by an exclusive borrow or by
/// a call whose callee may write its `self` receiver. Writes to a different
/// binding are not followed; their own `&mut symbol` creation is what dirties
/// `symbol`.
fn expression_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    root: ExpressionHandle,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
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
                    && place_interferes(program, machine_symbol, borrow.target, symbol, read_path)
                {
                    return true;
                }
                pending.push(borrow.target);
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid()
                    && place_interferes(program, machine_symbol, call.receiver, symbol, read_path)
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
                if place_interferes(program, machine_symbol, atomic.value, symbol, read_path) {
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
