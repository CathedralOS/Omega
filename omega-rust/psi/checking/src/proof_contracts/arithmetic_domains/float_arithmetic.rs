//! Named float arithmetic and float sources of integer casts.

use crate::proof_contracts::arithmetic_domains::integer_ranges::primitive_range;
use crate::proof_contracts::arithmetic_domains::place_paths::place_path;
use crate::proof_contracts::arithmetic_domains::value_environment::{
    FloatInterval, ValueEnvironment,
};
use crate::value_custody::places::declared_place_type_raw;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// Resolve only the reserved named-float arithmetic surface. Contract
/// expressions can reach validation before their static namespace receiver has
/// a symbol, so use the retained one-segment `F32`/`F64` spelling as a strict
/// fallback. The shared resolver still enforces exact path, arity, and
/// uniqueness; arbitrary float-returning operators do not enter this lane.
pub(crate) fn resolve_named_float_arithmetic<'program>(
    program: &'program TypedTrees,
    call: &TableCallExpression,
) -> Option<(
    &'program typed_trees::operator::OperatorDefinition,
    PrimitiveType,
)> {
    let operator =
        typed_trees::operator::resolve_named_expression_call(program, call).or_else(|| {
            let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let [namespace] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if !matches!(namespace.as_str(), "F32" | "F64") {
                return None;
            }
            let static_receiver = [namespace.as_str()];
            typed_trees::operator::resolve_named_call(
                program,
                call.target_symbol,
                Some(&static_receiver),
                call.target.as_str(),
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .len(),
                false,
            )
        })?;
    let primitive = program.primitive_type_reference(operator.return_type)?;
    let namespace = program.operator_path_members(operator.name).first()?;
    matches!(
        (namespace.as_str(), primitive),
        ("F32", PrimitiveType::F32) | ("F64", PrimitiveType::F64)
    )
    .then_some((operator, primitive))
}

/// The float value of a literal operand (through a `Mutable` wrapper), read at
/// its landed format, or `None` when the operand is not a plain float literal.
/// The F4 Exact cast obligation's fold-visible proof source.
pub(crate) fn float_literal_value(program: &TypedTrees, value: ExpressionHandle) -> Option<f64> {
    let mut node = program.expression_table.expression(value);
    while let ExpressionNode::Borrow(inner) = node {
        node = program.expression_table.expression(inner.target);
    }
    match node {
        ExpressionNode::Float(literal) => Some(literal.landed_f64()),
        _ => None,
    }
}

pub(crate) fn float_source_proves_int_cast(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    value: ExpressionHandle,
    target: PrimitiveType,
) -> bool {
    let (range, non_nan) = if let Some(literal) = float_literal_value(program, value) {
        (
            FloatInterval {
                low: Some(literal),
                high: Some(literal),
            },
            !literal.is_nan(),
        )
    } else {
        let Some(path) = place_path(program, value) else {
            return false;
        };
        let (flow_range, flow_non_nan) = environment.float_fact(&path);
        let declared = declared_place_type_raw(program, machine, state, value)
            .and_then(|handle| float_range_constraint_interval(program, handle));
        let range = declared
            .map(|declared| declared.intersect(flow_range))
            .unwrap_or(flow_range);
        let declared_finite = declared.is_some_and(|declared| {
            declared.low.is_some_and(f64::is_finite) && declared.high.is_some_and(f64::is_finite)
        });
        (range, flow_non_nan || declared_finite)
    };
    if !non_nan {
        return false;
    }
    let (Some(low), Some(high)) = (range.low, range.high) else {
        return false;
    };
    if !low.is_finite() || !high.is_finite() || low > high {
        return false;
    }
    float_interval_fits_integer(low.trunc(), high.trunc(), target)
}

pub(crate) fn float_interval_fits_integer(low: f64, high: f64, target: PrimitiveType) -> bool {
    // The i64/u64 upper endpoints need strict comparisons: `i64::MAX as f64`
    // rounds to 2^63, and 2^64 itself is likewise outside u64 even though it
    // is exactly representable as a float. Smaller integer endpoints are all
    // exactly representable in f64 and may use the ordinary closed interval.
    match target {
        PrimitiveType::I64 => low >= -9223372036854775808.0 && high < 9223372036854775808.0,
        PrimitiveType::U64 => low >= 0.0 && high < 18446744073709551616.0,
        _ => primitive_range(target).is_some_and(|range| {
            matches!((range.low, range.high), (Some(target_low), Some(target_high))
                if low >= target_low as f64 && high <= target_high as f64)
        }),
    }
}

pub(crate) fn float_range_constraint_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<FloatInterval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            float_range_constraint_interval(program, *referee)
        }
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                // An inclusive enclosure is conservative for source facts,
                // including a strict authored endpoint.
                TypeConstraintNode::Range {
                    minimum, maximum, ..
                } => Some(FloatInterval {
                    low: Some(float_literal_value(program, *minimum)?),
                    high: Some(float_literal_value(program, *maximum)?),
                }),
                _ => None,
            }),
        _ => None,
    }
}

pub(crate) fn float_bound_from(
    operator: BinaryOperator,
    literal: f64,
    name_on_left: bool,
) -> Option<FloatInterval> {
    let operator = if name_on_left {
        operator
    } else {
        match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            _ => return None,
        }
    };
    Some(match operator {
        // Treat strict bounds as inclusive at the same endpoint. This is a
        // conservative widening and avoids target-format nextafter logic.
        BinaryOperator::Less | BinaryOperator::LessOrEqual => FloatInterval {
            low: None,
            high: Some(literal),
        },
        BinaryOperator::Greater | BinaryOperator::GreaterOrEqual => FloatInterval {
            low: Some(literal),
            high: None,
        },
        _ => return None,
    })
}
