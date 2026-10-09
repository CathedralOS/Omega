//! Ordered float source enclosures from declared results and live place facts.
//! Exact coercions require integrality; named conversions may truncate. Both
//! consume the same source evidence without replaying operand syntax.

use crate::proof_contracts::arithmetic_domains::integer_ranges::primitive_range;
use crate::proof_contracts::arithmetic_domains::place_paths::place_path;
use crate::proof_contracts::arithmetic_domains::value_environment::{
    FloatInterval, ValueEnvironment,
};
use crate::value_custody::places::declared_place_type_raw;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// The float value of a literal operand (through a `Mutable` wrapper), read at
/// its landed format, or `None` when the operand is not a plain float literal.
pub(crate) fn float_literal_value(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
) -> Option<f64> {
    let node = program.expression_table.peeled_expression(value);
    match node {
        ExpressionNode::Float(literal) => Some(literal.landed_f64()),
        _ => None,
    }
}

pub(crate) fn float_source_proves_integer_conversion(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    value: ExpressionHandle,
    target: PrimitiveType,
) -> bool {
    float_source_bounds(program, machine, state, environment, value).is_some_and(|bounds| {
        bounds.minimum.is_finite()
            && bounds.maximum.is_finite()
            && float_interval_fits_integer(bounds.minimum.trunc(), bounds.maximum.trunc(), target)
    })
}

/// An ordered source enclosure at this occurrence. Declaration-backed values
/// and selected results share the canonical result-type query; live place facts
/// come from the ordinary checking walk, never from replaying initializers.
pub(crate) struct FloatSourceBounds {
    pub(crate) primitive: PrimitiveType,
    pub(crate) minimum: f64,
    pub(crate) maximum: f64,
}

impl FloatSourceBounds {
    pub(crate) fn is_exact_integer(&self, target: PrimitiveType) -> bool {
        self.is_integral() && float_interval_fits_integer(self.minimum, self.maximum, target)
    }

    pub(crate) fn is_integral(&self) -> bool {
        if !self.minimum.is_finite() || !self.maximum.is_finite() {
            return false;
        }
        if self.minimum == self.maximum {
            return self.minimum.trunc() == self.minimum;
        }
        // Above the significand's unit spacing, every representable value is
        // integral. Representable endpoints alone do not prove this below it.
        let unit_spacing = match self.primitive {
            PrimitiveType::F32 => 8388608.0,
            PrimitiveType::F64 => 4503599627370496.0,
            _ => return false,
        };
        self.minimum >= unit_spacing || self.maximum <= -unit_spacing
    }
}

pub(crate) fn float_source_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    value: ExpressionHandle,
) -> Option<FloatSourceBounds> {
    float_source_bounds_with_result_type(program, environment, value, &|value| {
        state
            .and_then(|state| {
                crate::value_custody::expression_types::expression_result_type_reference(
                    program, machine, state, value,
                )
            })
            .or_else(|| declared_place_type_raw(program, machine, state, value))
    })
}

/// The same source enclosure query for a declaration telescope without an
/// executable state. The callback supplies selected result types only; live
/// facts, literals and sealed format conversions keep their ordinary owner.
pub(crate) fn float_source_bounds_with_result_type(
    program: &dyn crate::CheckingInput,
    environment: &ValueEnvironment,
    value: ExpressionHandle,
    result_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
) -> Option<FloatSourceBounds> {
    let reference = result_type(value);
    let primitive = reference
        .and_then(|reference| program.primitive_type_reference(reference))
        .filter(|primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64))?;
    let declared =
        reference.and_then(|reference| float_range_constraint_interval(program, reference));
    let (flow, non_nan) = place_path(program, value)
        .map(|path| environment.float_fact(&path))
        .unwrap_or((FloatInterval::UNBOUNDED, false));
    let converted = selected_numeric_conversion_bounds(program, environment, value, result_type);
    let range = if let Some(value) = float_literal_value(program, value) {
        if value.is_nan() {
            return None;
        }
        FloatInterval {
            low: Some(value),
            high: Some(value),
        }
    } else {
        if declared.is_none() && converted.is_none() && !non_nan {
            return None;
        }
        let range = declared.map_or(flow, |declared| declared.intersect(flow));
        converted.map_or(range, |converted| range.intersect(converted))
    };
    let minimum = range.low?;
    let maximum = range.high?;
    (minimum <= maximum).then_some(FloatSourceBounds {
        primitive,
        minimum,
        maximum,
    })
}

/// A selected sealed numeric guarantee bounds the completed result. Integer
/// inputs use the selected kernel's full-carrier image; float inputs transport
/// their live ordered enclosure. Both conversions are monotone, and neither
/// destination annotations nor receiving predicates supply source bounds.
fn selected_numeric_conversion_bounds(
    program: &dyn crate::CheckingInput,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    result_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
) -> Option<FloatInterval> {
    let call = program.expression_table.call_node(expression)?;
    let operator = typed_trees::operator::resolve_named_expression_call(program, call)?;
    let (source, target) = crate::numeric_float_conversion_contract(program, operator)?;
    let [operand] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };
    if let Some((integer, converted, minimum, maximum)) =
        crate::integer_float_conversion_bounds(program, operator)
    {
        if integer != source
            || converted != target
            || result_type(*operand)
                .and_then(|reference| program.primitive_type_reference(reference))
                != Some(source)
        {
            return None;
        }
        return Some(FloatInterval {
            low: Some(minimum.to_f64()),
            high: Some(maximum.to_f64()),
        });
    }
    let bounds = float_source_bounds_with_result_type(program, environment, *operand, result_type)?;
    if bounds.primitive != source {
        return None;
    }
    let format = if target == PrimitiveType::F32 {
        numerics::float_semantics::FloatFormat::BINARY32
    } else {
        numerics::float_semantics::FloatFormat::BINARY64
    };
    let convert = |value| {
        numerics::float_semantics::FloatSemantics::convert(
            format,
            &numerics::float_semantics::FloatMeaning::from_f64(value),
        )
        .to_f64()
    };
    Some(FloatInterval {
        low: Some(convert(bounds.minimum)),
        high: Some(convert(bounds.maximum)),
    })
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
    program: &dyn crate::CheckingInput,
    handle: TypeReferenceHandle,
) -> Option<FloatInterval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            float_range_constraint_interval(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let primitive = program.primitive_type_reference(handle)?;
            let mut range = float_range_constraint_interval(program, *base_type);
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Range {
                    minimum, maximum, ..
                } = constraint
                {
                    let read = |expression| match crate::closed_float_range_endpoint(
                        program, expression, primitive,
                    )? {
                        semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                            Some(f64::from(f32::from_bits(bits)))
                        }
                        semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                            Some(f64::from_bits(bits))
                        }
                    };
                    let additional = FloatInterval {
                        low: Some(read(*minimum)?),
                        high: Some(read(*maximum)?),
                    };
                    range = Some(range.map_or(additional, |range| range.intersect(additional)));
                }
            }
            range
        }
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
            BinaryOperator::Equal => BinaryOperator::Equal,
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            _ => return None,
        }
    };
    Some(match operator {
        // This helper is reached only for a positive selected IEEE comparison.
        // Equality to a finite landed value excludes NaN and fixes the value.
        BinaryOperator::Equal => FloatInterval {
            low: Some(literal),
            high: Some(literal),
        },
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

/// Record a positively established IEEE comparison. The caller owns selected
/// builtin/sealed comparison meaning. Source place types and live facts remain
/// independent from the cast target, and negated comparisons do not enter this
/// operation because NaN prevents complementary ordered conclusions.
pub(super) fn record_float_comparison_bounds(
    program: &dyn crate::CheckingInput,
    environment: &mut ValueEnvironment,
    comparison: &typed_trees::expression::TableBinaryExpression,
    place_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
) -> bool {
    if comparison.operator == BinaryOperator::Equal
        && let (Some(left), Some(right)) = (
            place_path(program, comparison.left),
            place_path(program, comparison.right),
        )
        && left == right
        && place_type(comparison.left).is_some_and(|handle| {
            matches!(
                program.primitive_type_reference(handle),
                Some(PrimitiveType::F32 | PrimitiveType::F64)
            )
        })
    {
        environment.mark_non_nan(left);
        return true;
    }
    let float_sides = if let Some(literal) = float_literal_value(program, comparison.right) {
        Some((comparison.left, literal, true))
    } else {
        float_literal_value(program, comparison.left)
            .map(|literal| (comparison.right, literal, false))
    };
    if let Some((place_expression, literal, name_on_left)) = float_sides
        && literal.is_finite()
        && let Some(handle) = place_type(place_expression)
        && matches!(
            program.primitive_type_reference(handle),
            Some(PrimitiveType::F32 | PrimitiveType::F64)
        )
        && let Some(name) = place_path(program, place_expression)
        && let Some(mut interval) = float_bound_from(comparison.operator, literal, name_on_left)
    {
        if let Some(declared) = float_range_constraint_interval(program, handle) {
            interval = interval.intersect(declared);
        }
        environment.narrow_float(name.clone(), interval);
        environment.mark_non_nan(name);
        return true;
    }
    false
}
