//! Normal-completion bounds for builtin positive-literal counter updates.

use super::*;
use numerics::integer_policy::{IntegerPolicyPrimitive, IntegerResultLaw, integer_policy_bridge};

#[cfg(test)]
mod tests;

/// Transfer independently established bounds across `value + positive`,
/// `positive + value`, or `value - positive` when the actual builtin operation
/// preserves the corresponding non-strict order on normal completion.
///
/// The caller binds the nonliteral operand to its exact assignment target and
/// must not supply facts derived from this update's own monotonicity. This
/// query proves neither Exact formation nor the existence of a normal return.
/// Unknown endpoints stay unknown; in particular, the i64 interval owner does
/// not supply a finite maximum for the full u64 carrier.
pub fn builtin_monotonic_integer_update_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    lower: Option<i64>,
    upper: Option<i64>,
) -> Option<(Option<i64>, Option<i64>)> {
    if !crate::has_builtin_bound_expression_meaning(program, machine, Some(state), expression) {
        return None;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let (operand, literal) = match binary.operator {
        BinaryOperator::Add
            if matches!(
                program.expression_table.expression(binary.left),
                ExpressionNode::Integer(_)
            ) =>
        {
            (binary.right, binary.left)
        }
        BinaryOperator::Add | BinaryOperator::Subtract => (binary.left, binary.right),
        _ => return None,
    };
    let ExpressionNode::Integer(literal_value) = program.expression_table.expression(literal)
    else {
        return None;
    };
    let step = literal_value.value_i64().filter(|step| *step > 0)?;
    let operand_type = declared_place_type_raw(program, machine, Some(state), operand)?;
    let primitive = program.primitive_type_reference(operand_type)?;
    // Address offsets have a different operand/result contract from fixed-width
    // integer counter updates, even though their range also has a zero floor.
    if primitive == PrimitiveType::Addr {
        return None;
    }
    let carrier = primitive_range(primitive)?;
    if let Some(landing) = literal_value.landing() {
        let literal_type =
            crate::operators::landed_integer_literal_type_reference(program, literal)?;
        if program.primitive_type_reference(literal_type) != Some(primitive)
            || (landing.domain != ArithmeticDomain::Exact
                && landing.domain != program.arithmetic_domain_for_type_reference(operand_type))
        {
            return None;
        }
    }
    let step = Interval::constant(step);
    if !carrier.contains(step) {
        return None;
    }
    let before = Interval {
        low: lower,
        high: upper,
    }
    .intersect(carrier);
    if empty(before) {
        return None;
    }
    let (mathematical, policy_primitive) = match binary.operator {
        BinaryOperator::Add => (before.add(step), IntegerPolicyPrimitive::Add),
        BinaryOperator::Subtract => (before.subtract(step), IntegerPolicyPrimitive::Subtract),
        _ => return None,
    };
    let policy = program.arithmetic_domain_for_type_reference(operand_type);
    let result_law = integer_policy_bridge(policy_primitive, policy).result_law;
    let after = match result_law {
        IntegerResultLaw::Mathematical => mathematical.intersect(carrier),
        IntegerResultLaw::ClampToCarrier => {
            let mut clamped = mathematical;
            if let Some(minimum) = carrier.low {
                clamped = clamped.max_with(Interval::constant(minimum));
            }
            if let Some(maximum) = carrier.high {
                clamped = clamped.min_with(Interval::constant(maximum));
            }
            clamped
        }
        IntegerResultLaw::WrapToCarrier => {
            // An unknown u64 upper endpoint cannot establish that addition
            // avoids wrapping. Subtraction by a positive value cannot cross
            // the upper carrier boundary; it still must prove its lower end.
            if !carrier.contains(mathematical)
                || (binary.operator == BinaryOperator::Add && mathematical.high.is_none())
            {
                return None;
            }
            mathematical
        }
    };
    (!empty(after)).then_some((after.low, after.high))
}

fn empty(interval: Interval) -> bool {
    matches!((interval.low, interval.high), (Some(low), Some(high)) if low > high)
}
