//! Independent update snapshots for monotone loop proposals.
//!
//! The loop head is a cut: its bounds come from entry constants and actual
//! incoming guards, never from the monotonicity fact being proposed. Other
//! states receive the union of their evaluated arrivals. Widening loses a
//! bound rather than assuming that an unfinished iteration is inductive.
use super::{
    BinaryOperator, ExpressionHandle, Machine, State, StatementNode, SymbolHandle,
    TransitionGuardNode,
};
use crate::checks::ranges::loop_invariants::assignment_counter_field;
use crate::checks::ranges::loop_invariants::expression_is_counter_member;
use crate::checks::ranges::loop_invariants::integer_literal;
use crate::checks::ranges::loop_invariants::transition_target_symbol;
use validation::Interval;

pub(super) fn updates_preserve_order(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    states: &[State],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
    minimum_initial: i64,
    maximum_initial: i64,
) -> bool {
    let mut head_bounds = Interval {
        low: Some(minimum_initial),
        high: Some(maximum_initial),
    };
    for state in states
        .iter()
        .filter(|state| loop_states.contains(&state.symbol))
    {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for (target, positive) in [(transition.target, true), (transition.continuation, false)]
            {
                if transition_target_symbol(program, target) == Some(head) {
                    head_bounds = head_bounds.union(transition_bounds(
                        program,
                        machine,
                        state,
                        transition.guard,
                        counter,
                        positive,
                    ));
                }
            }
        }
    }

    let Some(head_index) = program.state_position_by_symbol(machine.symbol, head) else {
        return false;
    };
    let mut arrivals = vec![None; states.len()];
    let mut changes = vec![0usize; states.len()];
    let mut pending = vec![head_index];
    arrivals[head_index] = Some(head_bounds);
    while let Some(index) = pending.pop() {
        let state = &states[index];
        let Some(mut bounds) = arrivals[index] else {
            continue;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::Assignment(assignment) = statement
                && assignment_counter_field(program, machine, state, assignment) == Some(counter)
            {
                let Some((lower, upper)) = validation::builtin_monotonic_integer_update_bounds(
                    program,
                    machine,
                    state,
                    assignment.value,
                    bounds.low,
                    bounds.high,
                ) else {
                    return false;
                };
                bounds = Interval {
                    low: lower,
                    high: upper,
                };
            }
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for (target, positive) in [(transition.target, true), (transition.continuation, false)]
            {
                let Some(symbol) = transition_target_symbol(program, target) else {
                    continue;
                };
                if symbol == head || !loop_states.contains(&symbol) {
                    continue;
                }
                let Some(target_index) = program.state_position_by_symbol(machine.symbol, symbol)
                else {
                    return false;
                };
                let incoming = bounds.intersect(transition_bounds(
                    program,
                    machine,
                    state,
                    transition.guard,
                    counter,
                    positive,
                ));
                if matches!((incoming.low, incoming.high), (Some(lower), Some(upper)) if lower > upper)
                {
                    continue;
                }
                let next = match arrivals[target_index] {
                    None => incoming,
                    Some(previous) => {
                        let joined = previous.union(incoming);
                        // A widening endpoint is independently unknown; retain
                        // the other endpoint if it has already stabilized.
                        if changes[target_index] > states.len() {
                            Interval {
                                low: (joined.low == previous.low).then_some(joined.low).flatten(),
                                high: (joined.high == previous.high)
                                    .then_some(joined.high)
                                    .flatten(),
                            }
                        } else {
                            joined
                        }
                    }
                };
                if arrivals[target_index] != Some(next) {
                    arrivals[target_index] = Some(next);
                    changes[target_index] += 1;
                    if !pending.contains(&target_index) {
                        pending.push(target_index);
                    }
                }
            }
            // Like the ordinary range checker, reaching another statement
            // refutes a preceding exit arm with no explicit continuation.
            // This also preserves earlier arms in a flattened dispatch.
            if transition.target.is_valid() && !transition.continuation.is_valid() {
                bounds = bounds.intersect(transition_bounds(
                    program,
                    machine,
                    state,
                    transition.guard,
                    counter,
                    false,
                ));
                if matches!((bounds.low, bounds.high), (Some(lower), Some(upper)) if lower > upper)
                {
                    break;
                }
            }
        }
    }
    true
}

fn transition_bounds(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    guard: TransitionGuardNode,
    counter: SymbolHandle,
    positive: bool,
) -> Interval {
    match guard {
        TransitionGuardNode::Always => Interval::UNBOUNDED,
        TransitionGuardNode::When(guard) => {
            guard_bounds(program, machine, state, guard, counter, positive, 0)
        }
    }
}

fn guard_bounds(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    guard: ExpressionHandle,
    counter: SymbolHandle,
    positive: bool,
    depth: usize,
) -> Interval {
    if depth >= 128
        || !validation::has_builtin_decomposed_guard_meaning(program, machine, Some(state), guard)
    {
        return Interval::UNBOUNDED;
    }
    let Some(binary) = program.expression_table.binary_node(guard) else {
        // A subject spelled by name stands for its capture's initializer.
        let Some(subject) = super::boolean_subject_definition(program, state, guard) else {
            return Interval::UNBOUNDED;
        };
        return guard_bounds(
            program,
            machine,
            state,
            subject,
            counter,
            positive,
            depth + 1,
        );
    };
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        for (literal, inner) in [(binary.left, binary.right), (binary.right, binary.left)] {
            if let Some(value) = program.expression_table.boolean_node(literal) {
                let inner_positive =
                    (positive == *value) == (binary.operator == BinaryOperator::Equal);
                return guard_bounds(
                    program,
                    machine,
                    state,
                    inner,
                    counter,
                    inner_positive,
                    depth + 1,
                );
            }
        }
    }
    if (binary.operator == BinaryOperator::And && positive)
        || (binary.operator == BinaryOperator::Or && !positive)
    {
        return guard_bounds(
            program,
            machine,
            state,
            binary.left,
            counter,
            positive,
            depth + 1,
        )
        .intersect(guard_bounds(
            program,
            machine,
            state,
            binary.right,
            counter,
            positive,
            depth + 1,
        ));
    }
    let mut operator = binary.operator;
    let literal = if expression_is_counter_member(program, machine, state, binary.left, counter) {
        integer_literal(program, binary.right)
    } else if expression_is_counter_member(program, machine, state, binary.right, counter) {
        operator = operator.mirrored();
        integer_literal(program, binary.left)
    } else {
        return Interval::UNBOUNDED;
    };
    let Some(literal) = literal else {
        return Interval::UNBOUNDED;
    };
    if !positive {
        operator = match operator {
            BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
            BinaryOperator::LessOrEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessOrEqual,
            BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
            BinaryOperator::NotEqual => BinaryOperator::Equal,
            _ => return Interval::UNBOUNDED,
        };
    }
    match operator {
        BinaryOperator::Less => Interval {
            low: None,
            high: literal.checked_sub(1),
        },
        BinaryOperator::LessOrEqual => Interval {
            low: None,
            high: Some(literal),
        },
        BinaryOperator::Greater => Interval {
            low: literal.checked_add(1),
            high: None,
        },
        BinaryOperator::GreaterOrEqual => Interval {
            low: Some(literal),
            high: None,
        },
        BinaryOperator::Equal => Interval {
            low: Some(literal),
            high: Some(literal),
        },
        _ => Interval::UNBOUNDED,
    }
}
