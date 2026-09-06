//! Independent update snapshots for monotone loop proposals.
//!
//! The loop head is a cut: its bounds come from entry constants and actual
//! incoming guards, never from the monotonicity fact being proposed. Other
//! states receive the union of their evaluated arrivals. Widening loses a
//! bound rather than assuming that an unfinished iteration is inductive.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bounds {
    lower: Option<i64>,
    upper: Option<i64>,
}

impl Bounds {
    const UNKNOWN: Self = Self {
        lower: None,
        upper: None,
    };

    fn join(self, other: Self) -> Self {
        Self {
            lower: self
                .lower
                .zip(other.lower)
                .map(|(left, right)| left.min(right)),
            upper: self
                .upper
                .zip(other.upper)
                .map(|(left, right)| left.max(right)),
        }
    }

    fn intersect(self, other: Self) -> Self {
        Self {
            lower: match (self.lower, other.lower) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (left, right) => left.or(right),
            },
            upper: match (self.upper, other.upper) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (left, right) => left.or(right),
            },
        }
    }
}

pub(super) fn updates_preserve_order(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    states: &[State],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
    minimum_initial: i64,
    maximum_initial: i64,
) -> bool {
    let mut head_bounds = Bounds {
        lower: Some(minimum_initial),
        upper: Some(maximum_initial),
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
                    head_bounds = head_bounds.join(transition_bounds(
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

    let Some(head_index) = states.iter().position(|state| state.symbol == head) else {
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
                && assignment_counter_field(program, machine, assignment) == Some(counter)
            {
                let Some((lower, upper)) = validation::builtin_monotonic_integer_update_bounds(
                    program,
                    machine,
                    state,
                    assignment.value,
                    bounds.lower,
                    bounds.upper,
                ) else {
                    return false;
                };
                bounds = Bounds { lower, upper };
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
                let Some(target_index) = states.iter().position(|state| state.symbol == symbol)
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
                if matches!((incoming.lower, incoming.upper), (Some(lower), Some(upper)) if lower > upper)
                {
                    continue;
                }
                let next = match arrivals[target_index] {
                    None => incoming,
                    Some(previous) => {
                        let joined = previous.join(incoming);
                        // A widening endpoint is independently unknown; retain
                        // the other endpoint if it has already stabilized.
                        if changes[target_index] > states.len() {
                            Bounds {
                                lower: (joined.lower == previous.lower)
                                    .then_some(joined.lower)
                                    .flatten(),
                                upper: (joined.upper == previous.upper)
                                    .then_some(joined.upper)
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
                if matches!((bounds.lower, bounds.upper), (Some(lower), Some(upper)) if lower > upper)
                {
                    break;
                }
            }
        }
    }
    true
}

fn transition_bounds(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    guard: TransitionGuardNode,
    counter: SymbolHandle,
    positive: bool,
) -> Bounds {
    match guard {
        TransitionGuardNode::Always => Bounds::UNKNOWN,
        TransitionGuardNode::When(guard) => {
            guard_bounds(program, machine, state, guard, counter, positive, 0)
        }
    }
}

fn guard_bounds(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    guard: ExpressionHandle,
    counter: SymbolHandle,
    positive: bool,
    depth: usize,
) -> Bounds {
    if depth >= 128
        || !validation::has_builtin_decomposed_guard_meaning(program, machine, Some(state), guard)
    {
        return Bounds::UNKNOWN;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return Bounds::UNKNOWN;
    };
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        for (literal, inner) in [(binary.left, binary.right), (binary.right, binary.left)] {
            if let ExpressionNode::Boolean(value) = program.expression_table.expression(literal) {
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
    let literal = if expression_is_counter_member(program, machine, binary.left, counter) {
        integer_literal(program, binary.right)
    } else if expression_is_counter_member(program, machine, binary.right, counter) {
        operator = match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            other => other,
        };
        integer_literal(program, binary.left)
    } else {
        return Bounds::UNKNOWN;
    };
    let Some(literal) = literal else {
        return Bounds::UNKNOWN;
    };
    if !positive {
        operator = match operator {
            BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
            BinaryOperator::LessOrEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessOrEqual,
            BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
            BinaryOperator::NotEqual => BinaryOperator::Equal,
            _ => return Bounds::UNKNOWN,
        };
    }
    match operator {
        BinaryOperator::Less => Bounds {
            lower: None,
            upper: literal.checked_sub(1),
        },
        BinaryOperator::LessOrEqual => Bounds {
            lower: None,
            upper: Some(literal),
        },
        BinaryOperator::Greater => Bounds {
            lower: literal.checked_add(1),
            upper: None,
        },
        BinaryOperator::GreaterOrEqual => Bounds {
            lower: Some(literal),
            upper: None,
        },
        BinaryOperator::Equal => Bounds {
            lower: Some(literal),
            upper: Some(literal),
        },
        _ => Bounds::UNKNOWN,
    }
}
