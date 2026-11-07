//! Reading ranges back out of constraints: exactness, wrapping, finiteness
//! and the integer and float ranges a binary operation admits. Integer
//! operation bounds come from validation's shared arithmetic domain; this
//! client only translates the obligation's established constraint records.

use crate::obligations::plan::{ConstraintBuffer, FloatRange};
use crate::obligations::{IntegerRange, ProofConstraint};
use arena::HandleSpan;
use numerics::bignum::BigInt;
use typed_trees::expression::BinaryOperator;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn integer_constraints_are_exact(constraints: &ConstraintBuffer) -> bool {
    has_named_constraint(constraints, "exact")
        || integer_range_from_constraints(constraints).is_some()
}

pub(crate) fn integer_constraints_are_wrapping(constraints: &ConstraintBuffer) -> bool {
    has_named_constraint(constraints, "wrapping")
        || arithmetic_domain_from_constraints(constraints)
            == numerics::arithmetic::ArithmeticDomain::Wrapping
}

pub(crate) fn arithmetic_domain_from_constraints(
    constraints: &ConstraintBuffer,
) -> numerics::arithmetic::ArithmeticDomain {
    constraints
        .iter()
        .find_map(|constraint| match constraint {
            ProofConstraint::ArithmeticDomain(domain) => Some(*domain),
            _ => None,
        })
        .unwrap_or(numerics::arithmetic::ArithmeticDomain::Exact)
}

pub(crate) fn constraints_prove_finite(constraints: &ConstraintBuffer) -> bool {
    has_named_constraint(constraints, "finite")
        || float_range_from_constraints(constraints).is_some_and(|range| range.proves_finite())
}

pub(crate) fn has_named_constraint(constraints: &ConstraintBuffer, name: &str) -> bool {
    constraints.iter().any(|constraint| {
        matches!(
            constraint,
            ProofConstraint::Named(constraint_name) if constraint_name.as_str() == name
        )
    })
}

pub(crate) fn integer_range_from_constraints(
    constraints: &ConstraintBuffer,
) -> Option<IntegerRange> {
    integer_range_from_constraint_iter(constraints.iter())
}

pub(crate) fn integer_range_from_constraint_iter<'a>(
    constraints: impl Iterator<Item = &'a ProofConstraint>,
) -> Option<IntegerRange> {
    let constraints: Vec<&ProofConstraint> = constraints.collect();
    let mut range: Option<IntegerRange> = None;

    for constraint in constraints.iter() {
        let ProofConstraint::IntegerRange { minimum, maximum } = constraint else {
            continue;
        };

        let candidate = IntegerRange {
            minimum: minimum.clone(),
            maximum: maximum.clone(),
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    // Named sign facts RAISE an existing floor only. They used to fabricate
    // a standalone [0, i64::MAX] range -- a false upper claim for u64 atoms
    // (the widths carry the honest ranges now).
    for constraint in constraints.iter() {
        let ProofConstraint::Named(name) = constraint else {
            continue;
        };
        let floor = match name.as_str() {
            "non_negative" => BigInt::zero(),
            "positive" => BigInt::from_i64(1),
            _ => continue,
        };
        if let Some(existing) = range.as_mut()
            && existing.minimum < floor
        {
            existing.minimum = floor;
        }
    }

    range
}

pub(crate) fn float_range_from_constraints<'a>(
    constraints: impl IntoIterator<Item = &'a ProofConstraint>,
) -> Option<FloatRange> {
    let mut range: Option<FloatRange> = None;

    for constraint in constraints {
        let ProofConstraint::FloatRange {
            minimum,
            maximum,
            maximum_inclusive,
        } = constraint
        else {
            continue;
        };

        let candidate = FloatRange {
            minimum: minimum.landed_f64(),
            maximum: maximum.landed_f64(),
            maximum_inclusive: *maximum_inclusive,
        };

        range = Some(match range {
            Some(existing) => existing.intersect(candidate),
            None => candidate,
        });
    }

    range
}

pub(crate) use validation::integer_binary_range;

pub(crate) fn float_binary_range(
    operator: BinaryOperator,
    left: FloatRange,
    right: FloatRange,
) -> Option<FloatRange> {
    // A minted range asserts IEEE membership, which NaN never satisfies.
    // Provably-finite operands keep these operations NaN-free (magnitude
    // overflow still reaches +-inf, which remains an honest bound). Once an
    // operand's range admits an infinity the result can be NaN
    // (`inf + -inf`, `inf - inf`, `0 * inf`, `inf / inf`), and no honest
    // membership claim exists -- refuse rather than mint permissive evidence.
    if !left.proves_finite() || !right.proves_finite() {
        return None;
    }
    // Interval arithmetic over an open endpoint only weakens a conservative result.
    match operator {
        BinaryOperator::Add => Some(FloatRange {
            minimum: left.minimum + right.minimum,
            maximum: left.maximum + right.maximum,
            maximum_inclusive: true,
        }),
        BinaryOperator::Subtract => Some(FloatRange {
            minimum: left.minimum - right.maximum,
            maximum: left.maximum - right.minimum,
            maximum_inclusive: true,
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum * right.minimum,
                left.minimum * right.maximum,
                left.maximum * right.minimum,
                left.maximum * right.maximum,
            ];
            Some(FloatRange {
                minimum: products.iter().copied().fold(f64::INFINITY, f64::min),
                maximum: products.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                maximum_inclusive: true,
            })
        }
        BinaryOperator::Divide => {
            if right.minimum <= 0.0 && right.maximum >= 0.0 {
                return None;
            }

            let quotients = [
                left.minimum / right.minimum,
                left.minimum / right.maximum,
                left.maximum / right.minimum,
                left.maximum / right.maximum,
            ];
            Some(FloatRange {
                minimum: quotients.iter().copied().fold(f64::INFINITY, f64::min),
                maximum: quotients.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                maximum_inclusive: true,
            })
        }
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Modulo
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::CaseMembership => None,
    }
}

pub(crate) fn constrained_type_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, HandleSpan<TypeConstraintNode>)> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            constrained_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => Some((*base_type, *constraints)),
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => None,
    }
}
