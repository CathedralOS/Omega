//! Comparison facts declared in another scope, projected onto caller places.
//!
//! A case `where` fact speaks about the dispatched subject's fields; a
//! boundary callee's `ensures` speaks about its reference formals, which name
//! the caller's out-argument places once the call returns. Each owner
//! projects the two sides of one comparison onto caller operands; this module
//! contributes the projected comparison through the carriers an authored
//! guard produces: ordered relations (both orders for an equality, an
//! inequality relation for `!=`), place intervals for a literal side,
//! `known_u64` values, and joint-subtract bounds. A side the projection could
//! not identify contributes nothing -- a missed premise is sound, a wrong one
//! is not. Ordinary write invalidation retires every contributed fact.

use super::super::TypeReferenceHandle;
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, Interval, PrimitiveType, TypedTrees,
    ValueEnvironment, comparison_interval, literal_i64, ordered_values, primitive_range,
    range_constraint_interval,
};

/// One side of a projected comparison. `operand` is the symbol-exact
/// ordered-values identity, present only when the side has fixed-integer
/// meaning (a strict integer order has a gap of at least one; a float order
/// does not). `spelled` and `declared_type` key the spelling-based interval
/// carriers; `spelled` is `None` when the spelling could name another place.
pub(super) struct ProjectedOperand {
    pub(super) operand: Option<ordered_values::Operand>,
    pub(super) spelled: Option<String>,
    pub(super) declared_type: Option<TypeReferenceHandle>,
}

impl ProjectedOperand {
    pub(super) fn literal(literal: &numerics::literals::IntegerLiteral) -> Self {
        Self {
            operand: Some(ordered_values::Operand::Integer(
                literal.clone(),
                literal.landing(),
            )),
            spelled: None,
            declared_type: None,
        }
    }
}

/// Contribute `left <operator> right`. `left_expression`/`right_expression`
/// are the fact's own side expressions; only their literal values are read.
pub(super) fn contribute_projected_comparison(
    program: &TypedTrees,
    environment: &mut ValueEnvironment,
    operator: BinaryOperator,
    (left, left_expression): (&ProjectedOperand, ExpressionHandle),
    (right, right_expression): (&ProjectedOperand, ExpressionHandle),
) {
    match operator {
        BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            // `record`'s normalization: `high - low >= floor`, floor 1 for
            // the strict forms.
            let (high, low, strict) = match operator {
                BinaryOperator::LessOrEqual => (right, left, false),
                BinaryOperator::Less => (right, left, true),
                BinaryOperator::GreaterOrEqual => (left, right, false),
                _ => (left, right, true),
            };
            push_order(environment, high, low, i64::from(strict));
            // The unsigned `low <= high` premise is exactly the totality
            // condition for `high - low` -- `joint_subtract_guard`'s carrier.
            if !strict
                && let (Some(high_path), Some(low_path)) =
                    (high.spelled.as_ref(), low.spelled.as_ref())
                && let (Some(high_type), Some(low_type)) = (high.declared_type, low.declared_type)
                && program
                    .primitive_type_reference(high_type)
                    .is_some_and(|primitive| {
                        matches!(
                            primitive,
                            PrimitiveType::U8
                                | PrimitiveType::U16
                                | PrimitiveType::U32
                                | PrimitiveType::U64
                        ) && program.primitive_type_reference(low_type) == Some(primitive)
                            && program.arithmetic_domain_for_type_reference(high_type)
                                == ArithmeticDomain::Exact
                            && program.arithmetic_domain_for_type_reference(low_type)
                                == ArithmeticDomain::Exact
                    })
            {
                environment.mark_joint_subtract_bound(high_path.clone(), low_path.clone());
            }
            narrow_literal_sides(
                program,
                environment,
                operator,
                (left, left_expression),
                (right, right_expression),
            );
        }
        BinaryOperator::Equal => {
            // Builtin integer equality contributes both non-strict orders,
            // kept as relations so a write to either side retires it.
            push_order(environment, left, right, 0);
            push_order(environment, right, left, 0);
            narrow_literal_sides(
                program,
                environment,
                operator,
                (left, left_expression),
                (right, right_expression),
            );
            for (place, literal_expression) in [(left, right_expression), (right, left_expression)]
            {
                if let (Some(path), Some(declared_type), Some(literal)) = (
                    place.spelled.as_ref(),
                    place.declared_type,
                    literal_i64(program, literal_expression),
                ) && program.primitive_type_reference(declared_type) == Some(PrimitiveType::U64)
                    && let Ok(value) = u64::try_from(literal)
                {
                    environment.mark_known_u64(path.clone(), value);
                }
            }
        }
        BinaryOperator::NotEqual => {
            if let (Some(left), Some(right)) = (&left.operand, &right.operand) {
                let relation = ordered_values::Relation {
                    left: left.clone(),
                    right: right.clone(),
                    constraint: ordered_values::RelationConstraint::Unequal,
                };
                if !environment.ordered_values.contains(&relation) {
                    environment.ordered_values.push(relation);
                }
            }
        }
        _ => {}
    }
}

fn push_order(
    environment: &mut ValueEnvironment,
    high: &ProjectedOperand,
    low: &ProjectedOperand,
    floor: i64,
) {
    let (Some(high), Some(low)) = (&high.operand, &low.operand) else {
        return;
    };
    let relation = ordered_values::Relation {
        left: high.clone(),
        right: low.clone(),
        constraint: ordered_values::RelationConstraint::MinimumDifference(floor),
    };
    if !environment.ordered_values.contains(&relation) {
        environment.ordered_values.push(relation);
    }
}

/// A literal side narrows the place side's interval, intersected with the
/// place's type and declared ranges exactly like a guard leaf.
fn narrow_literal_sides(
    program: &TypedTrees,
    environment: &mut ValueEnvironment,
    operator: BinaryOperator,
    (left, left_expression): (&ProjectedOperand, ExpressionHandle),
    (right, right_expression): (&ProjectedOperand, ExpressionHandle),
) {
    for (place, literal_expression, place_on_left) in [
        (left, right_expression, true),
        (right, left_expression, false),
    ] {
        let (Some(path), Some(literal)) = (
            place.spelled.as_ref(),
            literal_i64(program, literal_expression),
        ) else {
            continue;
        };
        let mut interval = comparison_interval(
            operator,
            Interval {
                low: Some(literal),
                high: Some(literal),
            },
            place_on_left,
        );
        if let Some(declared_type) = place.declared_type {
            if let Some(type_interval) = program
                .primitive_type_reference(declared_type)
                .and_then(primitive_range)
            {
                interval = interval.intersect(type_interval);
            }
            if let Some(declared) = range_constraint_interval(program, declared_type) {
                interval = interval.intersect(declared);
            }
        }
        environment.narrow(path.clone(), interval);
    }
}

/// Whether a projected side may join ordered relations: its declared type is
/// a fixed-width integer, so a strict order is a gap of at least one.
pub(super) fn has_fixed_integer_meaning(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    crate::value_custody::places::unwrapped_type_reference(program, type_reference)
        .and_then(|reference| {
            crate::value_custody::recasts::exact_primitive_type(program, reference)
        })
        .is_some_and(|primitive| primitive.is_fixed_integer())
}
