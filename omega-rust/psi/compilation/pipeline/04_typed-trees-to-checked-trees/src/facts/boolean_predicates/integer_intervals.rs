//! Numeric consequences of a surviving proposition, over its exact bindings.
//! Conjunction intersects; alternatives hull. Unknown clauses keep the carrier,
//! and contradictory clauses never supply an arbitrary interval. Opposite
//! operands use ordinary completed values, never this proposition recursively.

use super::{BoundPredicate, Operand};
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
};
use facts::{IntegerRange, ScalarValue};
use numerics::bignum::BigInt;
use typed_trees::types::PrimitiveType;
use validation::CanonicalPlace;

impl BoundPredicate {
    pub(crate) fn integer_interval_at_place(
        &self,
        place: &CanonicalPlace,
        primitive: PrimitiveType,
        mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
    ) -> Option<IntegerRange> {
        let carrier = validation::carrier_integer_range(primitive)?;
        consequence(
            self,
            &self.expression,
            false,
            place,
            &carrier,
            &mut ordinary,
        )
    }
}

fn consequence(
    predicate: &BoundPredicate,
    expression: &Boolean,
    negated: bool,
    place: &CanonicalPlace,
    carrier: &IntegerRange,
    ordinary: &mut impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<IntegerRange> {
    let mut bounds =
        match expression {
            Boolean::Not(operand) => {
                return consequence(predicate, operand, !negated, place, carrier, ordinary);
            }
            Boolean::And { left, right } | Boolean::Or { left, right } => {
                let left = consequence(predicate, left, negated, place, carrier, ordinary);
                let right = consequence(predicate, right, negated, place, carrier, ordinary);
                let conjunction = matches!(expression, Boolean::And { .. }) != negated;
                if conjunction {
                    let (left, right) = (left?, right?);
                    IntegerRange {
                        minimum: left.minimum.max(right.minimum),
                        maximum: left.maximum.min(right.maximum),
                    }
                } else {
                    match (left, right) {
                        (Some(left), Some(right)) => IntegerRange {
                            minimum: left.minimum.min(right.minimum),
                            maximum: left.maximum.max(right.maximum),
                        },
                        (left, right) => return left.or(right),
                    }
                }
            }
            Boolean::Constant(value) if *value == negated => return None,
            Boolean::IntegerComparison { kind, left, right } => {
                let selected = [(left, right, false), (right, left, true)]
                    .into_iter()
                    .find_map(|(subject, value, reversed)| {
                        let binding = predicate.binding(&Operand::Scalar((**subject).clone()))?;
                        if binding.place != *place {
                            return None;
                        }
                        let value = crate::values::evaluate_checked_scalar(
                            value,
                            &mut super::Values {
                                predicate,
                                resolve: &mut *ordinary,
                            },
                        )?;
                        let ScalarValue::Integer(_) = value else {
                            return None;
                        };
                        Some((value, reversed))
                    });
                let Some((value, reversed)) = selected else {
                    return Some(carrier.clone());
                };
                let ScalarValue::Integer(value) = value else {
                    return None;
                };
                let one = BigInt::from_u64(1);
                let mut bounds = carrier.clone();
                match (kind, reversed, negated) {
                    (Comparison::Equal, _, false) => {
                        bounds.minimum = value.clone();
                        bounds.maximum = value;
                    }
                    (Comparison::Equal, _, true) => {}
                    (Comparison::LessThan, false, false)
                    | (Comparison::LessOrEqual, true, true) => bounds.maximum = value.sub(&one),
                    (Comparison::LessOrEqual, false, false)
                    | (Comparison::LessThan, true, true) => bounds.maximum = value,
                    (Comparison::LessThan, true, false)
                    | (Comparison::LessOrEqual, false, true) => bounds.minimum = value.add(&one),
                    (Comparison::LessOrEqual, true, false)
                    | (Comparison::LessThan, false, true) => bounds.minimum = value,
                }
                bounds
            }
            _ => carrier.clone(),
        };
    bounds.minimum = bounds.minimum.max(carrier.minimum.clone());
    bounds.maximum = bounds.maximum.min(carrier.maximum.clone());
    (bounds.minimum <= bounds.maximum).then_some(bounds)
}
