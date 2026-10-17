//! Length of a slice value, snapshotted when the slice is bound.
//!
//! A runtime window `items[start..end]` has length `end - start` once that
//! difference is a proved nonnegative integer. The binding records that
//! interval under `place.len` at the moment the endpoints are evaluated.
//! Later reads, copies, fields, and calls use the snapshot. Replaying the
//! initializer after an endpoint changes would describe a different window.

use super::ordered_values::{self, Operand, Relation, RelationConstraint};
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, State, ValueEnvironment,
};
use crate::value_custody::places::{
    declared_collection_extent, has_builtin_subslice_meaning, subslice_expression,
};
use numerics::literals::IntegerLiteral;

/// Record `destination.len` when `initializer` is a builtin subslice whose
/// length interval is already proved. A copy of an existing slice keeps the
/// snapshot the copy owner already translated; this function does not replay
/// that initializer.
pub(crate) fn publish_slice_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    destination: &str,
    destination_symbol: symbols::SymbolHandle,
    initializer: ExpressionHandle,
) {
    let Some(interval) = slice_length_interval(program, machine, state, environment, initializer)
    else {
        return;
    };
    environment.set(format!("{destination}.len"), interval);
    let Some(state) = state else {
        return;
    };
    if !destination_symbol.is_valid() {
        return;
    }
    let Some(measure) = length_measure(program, machine, state, initializer) else {
        return;
    };
    relate_equal(
        environment,
        Operand::CollectionLength(Box::new(Operand::Place {
            root: destination_symbol,
            path: destination.to_owned(),
        })),
        measure,
    );
}

/// The collection length this expression denotes, including a call whose
/// `ensures` equates the result with an argument's length and a match whose
/// every arm denotes that same length.
pub(super) fn denoted_collection_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
) -> Option<Operand> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            let mut shared = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let arm_length =
                    denoted_collection_length(program, machine, state, environment, arm.value)?;
                match &shared {
                    None => shared = Some(arm_length),
                    Some(length) if length != &arm_length => return None,
                    Some(_) => {}
                }
            }
            shared
        }
        ExpressionNode::Call(call) => measured_call_length(program, machine, state, call),
        _ => {
            let observed = ordered_values::operand(program, machine, state, expression, None)?;
            collection_length_of(environment, observed)
        }
    }
}

/// The length interval carried by this collection expression: a stored
/// `place.len` snapshot, or the length of a subslice expression itself.
pub(crate) fn carried_slice_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    collection: ExpressionHandle,
) -> Option<Interval> {
    let collection = strip_borrow(program, collection);
    if let Some(path) = super::place_path(program, collection) {
        if let Some(interval) = environment.get(&format!("{path}.len")) {
            return Some(interval);
        }
    }
    slice_length_interval(program, machine, state, environment, collection)
}

fn slice_length_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
) -> Option<Interval> {
    let operation = subslice_expression(program, expression)?;
    if !has_builtin_subslice_meaning(program, machine, state, operation) {
        return None;
    }
    let indexed = program.expression_table.indexed_node(operation)?;
    let range = program.expression_table.range_node(indexed.index)?;
    let start = if range.start.is_valid() {
        observed_integer_interval(program, environment, range.start)
    } else {
        Interval::constant(0)
    };
    let mut end = if range.end.is_valid() {
        observed_integer_interval(program, environment, range.end)
    } else if range.end_inclusive {
        return None;
    } else {
        collection_extent_interval(program, machine, state, environment, indexed.collection)?
    };
    if range.end_inclusive {
        end = end.add(Interval::constant(1));
    }
    nonnegative_difference(end, start)
}

fn collection_extent_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    collection: ExpressionHandle,
) -> Option<Interval> {
    let collection = strip_borrow(program, collection);
    if let Some(extent) = declared_collection_extent(program, machine, state, collection) {
        return i64::try_from(extent).ok().map(Interval::constant);
    }
    let path = super::place_path(program, collection)?;
    environment.get(&format!("{path}.len"))
}

fn observed_integer_interval(
    program: &dyn crate::CheckingInput,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
) -> Interval {
    if let Some(value) = super::integer_ranges::literal_i64(program, expression) {
        return Interval::constant(value);
    }
    let Some(path) = super::place_path(program, expression) else {
        return Interval::UNBOUNDED;
    };
    environment.get(&path).unwrap_or(Interval::UNBOUNDED)
}

/// `end - start` when every value of that difference is nonnegative. A
/// negative low means the window is not proved to exist, so no length fact
/// is published. An unknown low is not rewritten into zero: zero would not
/// be a new fact, and a positive low must come from the endpoints.
fn nonnegative_difference(end: Interval, start: Interval) -> Option<Interval> {
    let low = end.low?.checked_sub(start.high?)?;
    if low < 0 {
        return None;
    }
    let high = match (end.high, start.low) {
        (Some(end_high), Some(start_low)) => end_high.checked_sub(start_low),
        _ => None,
    };
    if high.is_some_and(|high| high < low) {
        return None;
    }
    Some(Interval {
        low: Some(low),
        high,
    })
}

fn strip_borrow(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> ExpressionHandle {
    for _ in 0..8 {
        let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
            break;
        };
        if !matches!(
            borrow.access,
            language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
        ) {
            break;
        }
        expression = borrow.target;
    }
    expression
}

fn collection_length_of(environment: &ValueEnvironment, observed: Operand) -> Option<Operand> {
    if matches!(observed, Operand::CollectionLength(_)) {
        return Some(observed);
    }
    environment.ordered_values.iter().find_map(|relation| {
        let Operand::CollectionLength(_) = &relation.right else {
            return None;
        };
        (relation.left == observed
            && relation.constraint == RelationConstraint::MinimumDifference(0)
            && environment.ordered_values.iter().any(|opposite| {
                opposite.constraint == RelationConstraint::MinimumDifference(0)
                    && opposite.left == relation.right
                    && opposite.right == relation.left
            }))
        .then(|| relation.right.clone())
    })
}

fn measured_call_length(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<Operand> {
    let collection = super::normal_return_length_collection(program, call)?;
    let collection = strip_borrow(program, collection);
    let receiver = ordered_values::operand(program, machine, state, collection, None)?;
    Some(Operand::CollectionLength(Box::new(receiver)))
}

/// `end - start` as an ordered operand. A literal zero start is the end
/// itself: `items[0..count]` has the same length as `count`.
fn length_measure(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Operand> {
    let operation = subslice_expression(program, expression)?;
    if !has_builtin_subslice_meaning(program, machine, Some(state), operation) {
        return None;
    }
    let indexed = program.expression_table.indexed_node(operation)?;
    let range = program.expression_table.range_node(indexed.index)?;
    let start = if range.start.is_valid() {
        ordered_values::operand(program, machine, state, range.start, None)?
    } else {
        integer_operand(0)
    };
    let end = if range.end.is_valid() {
        ordered_values::operand(program, machine, state, range.end, None)?
    } else if range.end_inclusive {
        return None;
    } else {
        let collection = strip_borrow(program, indexed.collection);
        Operand::CollectionLength(Box::new(ordered_values::operand(
            program, machine, state, collection, None,
        )?))
    };
    let mut measure = if is_zero(&start) {
        end
    } else {
        binary_operand(BinaryOperator::Subtract, end, start)
    };
    if range.end_inclusive {
        measure = binary_operand(BinaryOperator::Add, measure, integer_operand(1));
    }
    Some(measure)
}

fn integer_operand(value: i64) -> Operand {
    Operand::Integer(IntegerLiteral::from_value(value), None)
}

fn is_zero(operand: &Operand) -> bool {
    matches!(operand, Operand::Integer(literal, _) if literal.value_i64() == Some(0))
}

fn binary_operand(operator: BinaryOperator, left: Operand, right: Operand) -> Operand {
    Operand::Binary {
        operator,
        primitive: PrimitiveType::U64,
        domain: ArithmeticDomain::Exact,
        operands: vec![left, right],
    }
}

fn relate_equal(environment: &mut ValueEnvironment, left: Operand, right: Operand) {
    if left == right {
        return;
    }
    for (left, right) in [(left.clone(), right.clone()), (right, left)] {
        let relation = Relation {
            left,
            right,
            constraint: RelationConstraint::MinimumDifference(0),
        };
        if !environment.ordered_values.contains(&relation) {
            environment.ordered_values.push(relation);
        }
    }
}
