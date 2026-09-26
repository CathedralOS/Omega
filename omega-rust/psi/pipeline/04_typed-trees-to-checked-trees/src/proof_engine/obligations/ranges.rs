//! Reading ranges back out of constraints: exactness, wrapping, finiteness
//! and the integer and float ranges a binary operation admits.

use crate::proof_engine::obligations::plan::{ConstraintBuffer, FloatRange};
use crate::proof_engine::obligations::{IntegerRange, ProofConstraint};
use arena::HandleSpan;
use numerics::bignum::BigInt;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

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

pub(crate) fn float_range_from_constraints(constraints: &ConstraintBuffer) -> Option<FloatRange> {
    let mut range: Option<FloatRange> = None;

    for constraint in constraints.iter() {
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

pub(crate) fn integer_binary_range(
    operator: BinaryOperator,
    left: IntegerRange,
    right: IntegerRange,
) -> Option<IntegerRange> {
    let one = BigInt::from_i64(1);
    match operator {
        BinaryOperator::Add => Some(IntegerRange {
            minimum: left.minimum.add(&right.minimum),
            maximum: left.maximum.add(&right.maximum),
        }),
        BinaryOperator::Subtract => Some(IntegerRange {
            minimum: left.minimum.sub(&right.maximum),
            maximum: left.maximum.sub(&right.minimum),
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum.mul(&right.minimum),
                left.minimum.mul(&right.maximum),
                left.maximum.mul(&right.minimum),
                left.maximum.mul(&right.maximum),
            ];
            Some(IntegerRange {
                minimum: products.iter().min()?.clone(),
                maximum: products.iter().max()?.clone(),
            })
        }
        BinaryOperator::Modulo => {
            if right.minimum <= BigInt::zero() && right.maximum >= BigInt::zero() {
                return None;
            }
            let magnitude = right.minimum.abs().max(right.maximum.abs()).sub(&one);
            // Truncating remainder follows the dividend's sign, independently
            // of the divisor's sign. Its magnitude cannot exceed the dividend.
            Some(IntegerRange {
                minimum: left.minimum.min(BigInt::zero()).max(magnitude.negate()),
                maximum: left.maximum.max(BigInt::zero()).min(magnitude),
            })
        }
        BinaryOperator::ShiftRight => {
            if right.minimum.is_negative() {
                return None;
            }

            Some(IntegerRange {
                minimum: BigInt::zero().max(left.minimum.clone()),
                maximum: left.maximum.clone().max(BigInt::zero()),
            })
        }
        BinaryOperator::Divide => {
            // The divisor must provably exclude 0: entirely positive or
            // entirely negative. On a single-signed divisor interval the four
            // corner quotients are extremal for truncating division (x/k is
            // monotone in x for fixed k, and piecewise monotone in k on one
            // sign side), so min/max over the corners is exact --
            // `[0..=259] / 26` folds to `[0..=9]`, which used to return None
            // and reject a provably-in-range store. (Exact bignum: the old
            // `i64::MIN / -1` overflow bail is gone.)
            if !(right.minimum >= one || right.maximum <= one.negate()) {
                return None;
            }
            let corners = [
                left.minimum.div_rem(&right.minimum)?.0,
                left.minimum.div_rem(&right.maximum)?.0,
                left.maximum.div_rem(&right.minimum)?.0,
                left.maximum.div_rem(&right.maximum)?.0,
            ];
            Some(IntegerRange {
                minimum: corners.iter().min()?.clone(),
                maximum: corners.iter().max()?.clone(),
            })
        }
        BinaryOperator::BitwiseAnd => {
            // `x & mask` with BOTH operands provably non-negative: an AND
            // never sets a bit absent from either operand, so the result is
            // in [0, min(left.max, right.max)]. A possibly-negative operand
            // (sign bits) stays unfolded.
            if left.minimum.is_negative() || right.minimum.is_negative() {
                return None;
            }
            Some(IntegerRange {
                minimum: BigInt::zero(),
                maximum: left.maximum.min(right.maximum),
            })
        }
        BinaryOperator::And
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::CaseMembership => None,
    }
}

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

/// The proof window for one authored floating range endpoint, read at the
/// range's DECLARED carrier (`crate::validation::closed_float_range_endpoint`'s
/// twin). An f32 endpoint reads its spelling directly at binary32 and widens
/// exactly into the f64 window -- the widened f32 grid is what every landed
/// argument compares against -- while an f64 endpoint keeps its authored
/// landing (`0.3f32` under `f64[...]` means the widened f32 value). An
/// f64-landed literal cannot narrow through the f32 read, so it is not an
/// endpoint at all. Integer endpoints convert once into the carrier, exactly
/// as the retained interchange bits do.
pub(crate) fn float_range_bound(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Option<f64> {
    let carrier = program.primitive_type_reference(base_type);
    match program.expression_table.expression(expression) {
        ExpressionNode::Float(value) => match carrier {
            Some(PrimitiveType::F32) => (value.landing()
                != Some(numerics::literals::FloatFormat::F64))
            .then(|| f64::from(value.value_f32())),
            _ => Some(value.landed_f64()),
        },
        // Mixed floating ranges may have integer endpoints, but that does not
        // erase their selected landing or authorize a same-spelled constant.
        // A failed exact endpoint cannot fall back to its raw literal text.
        ExpressionNode::Integer(_) | ExpressionNode::Name(_) => {
            crate::validation::closed_integer_range_bound(program, expression)
                .and_then(|value| value.to_i64())
                .map(|value| match carrier {
                    Some(PrimitiveType::F32) => f64::from(value as f32),
                    _ => value as f64,
                })
        }
        _ => None,
    }
}

pub(crate) fn constrained_type_reference(
    program: &TypedTrees,
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
