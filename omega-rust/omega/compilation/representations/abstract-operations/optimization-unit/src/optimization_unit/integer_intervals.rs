//! Closed conjunctive integer-interval extraction from canonical propositions,
//! plus the exact proof-bearing operations whose accepted obligations carry them.
use abstract_operations::{
    AbstractOperation as O, IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, OperationId, Proposition, ScalarTerm,
    ScalarType, ValueId,
};
use std::collections::BTreeMap;
use terminal_semantics::CanonicalScalarGoal;

#[derive(Debug, Clone, Copy, Default)]
pub struct PartialIntegerBounds {
    pub lower: Option<IntegerValue>,
    pub upper: Option<IntegerValue>,
}

pub enum IntervalExtraction {
    Unsupported,
    Contradiction,
    Bounds(BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>),
}

pub fn extract_integer_intervals(proposition: &Proposition) -> IntervalExtraction {
    if proposition.validate().is_err() {
        return IntervalExtraction::Unsupported;
    }
    let mut bounds = BTreeMap::new();
    if !extract_integer_interval_conjunct(proposition, &mut bounds) {
        return IntervalExtraction::Contradiction;
    }
    for ((_, scalar_type), interval) in &bounds {
        let minimum = interval
            .lower
            .unwrap_or_else(|| scalar_type.minimum_value());
        let maximum = interval
            .upper
            .unwrap_or_else(|| scalar_type.maximum_value());
        if integer_value_cmp(*scalar_type, minimum, maximum).is_none_or(|order| order.is_gt()) {
            return IntervalExtraction::Contradiction;
        }
    }
    if bounds.is_empty() {
        IntervalExtraction::Unsupported
    } else {
        IntervalExtraction::Bounds(bounds)
    }
}

fn extract_integer_interval_conjunct(
    proposition: &Proposition,
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
) -> bool {
    match proposition {
        Proposition::Truth => true,
        Proposition::Falsehood => false,
        Proposition::Conjunction(conjuncts) => conjuncts
            .iter()
            .all(|conjunct| extract_integer_interval_conjunct(conjunct, bounds)),
        Proposition::Equal(left, right) => {
            let Some((value, scalar_type, literal)) =
                value_and_literal(left, right).or_else(|| value_and_literal(right, left))
            else {
                return true;
            };
            merge_lower(bounds, value, scalar_type, literal)
                && merge_upper(bounds, value, scalar_type, literal)
        }
        Proposition::LessOrEqual(left, right) => {
            if let Some((value, scalar_type, literal)) = value_and_literal(left, right) {
                merge_upper(bounds, value, scalar_type, literal)
            } else if let Some((value, scalar_type, literal)) = value_and_literal(right, left) {
                merge_lower(bounds, value, scalar_type, literal)
            } else {
                true
            }
        }
        Proposition::LessThan(left, right) => {
            if let Some((value, scalar_type, literal)) = value_and_literal(left, right) {
                let Some(upper) = predecessor(scalar_type, literal) else {
                    return false;
                };
                merge_upper(bounds, value, scalar_type, upper)
            } else if let Some((value, scalar_type, literal)) = value_and_literal(right, left) {
                let Some(lower) = successor(scalar_type, literal) else {
                    return false;
                };
                merge_lower(bounds, value, scalar_type, lower)
            } else {
                true
            }
        }
        Proposition::Atom(_)
        | Proposition::IntegerMathEqual(_, _)
        | Proposition::IntegerMathLessThan(_, _)
        | Proposition::IntegerMathLessOrEqual(_, _)
        | Proposition::IeeeFloatComparison { .. }
        | Proposition::ScalarIeeeFloatComparison { .. }
        | Proposition::ByteSequenceEqual { .. }
        | Proposition::StructuralIdentity { .. }
        | Proposition::StructuralCaseMembership { .. }
        | Proposition::Disjunction(_)
        | Proposition::Implication { .. }
        | Proposition::ContentConservation(_) => true,
    }
}

fn value_and_literal(
    value: &ScalarTerm,
    literal: &ScalarTerm,
) -> Option<(ValueId, IntegerType, IntegerValue)> {
    let ScalarTerm::Value {
        id,
        scalar_type: ScalarType::Integer(value_type),
    } = value
    else {
        return None;
    };
    let ScalarTerm::Integer {
        scalar_type: literal_type,
        value,
    } = literal
    else {
        return None;
    };
    (*value_type == *literal_type
        && value_type.carrier() == IntegerCarrier::Fixed
        && value_type.admits(*value))
    .then_some((*id, *value_type, *value))
}

fn merge_lower(
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
    value: ValueId,
    scalar_type: IntegerType,
    lower: IntegerValue,
) -> bool {
    let interval = bounds.entry((value, scalar_type)).or_default();
    interval.lower = match interval.lower {
        None => Some(lower),
        Some(current) => match integer_value_cmp(scalar_type, current, lower) {
            Some(order) if order.is_lt() => Some(lower),
            Some(_) => Some(current),
            None => return false,
        },
    };
    true
}

fn merge_upper(
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
    value: ValueId,
    scalar_type: IntegerType,
    upper: IntegerValue,
) -> bool {
    let interval = bounds.entry((value, scalar_type)).or_default();
    interval.upper = match interval.upper {
        None => Some(upper),
        Some(current) => match integer_value_cmp(scalar_type, current, upper) {
            Some(order) if order.is_gt() => Some(upper),
            Some(_) => Some(current),
            None => return false,
        },
    };
    true
}

pub(crate) fn integer_value_cmp(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<std::cmp::Ordering> {
    if !scalar_type.admits(left) || !scalar_type.admits(right) {
        return None;
    }
    match (scalar_type.sign(), left, right) {
        (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            Some(left.cmp(&right))
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            Some(left.cmp(&right))
        }
        _ => None,
    }
}

fn predecessor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = scalar_type.one();
    scalar_type.exact_sub(value, one)
}

fn successor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = scalar_type.one();
    scalar_type.exact_add(value, one)
}

pub fn proof_range_goal(
    operation: &O,
) -> Option<(
    OperationId,
    semantic_vocabulary::ObligationId,
    CanonicalScalarGoal,
)> {
    let value_term = |id, scalar_type| ScalarTerm::value(id, ScalarType::Integer(scalar_type));
    match operation {
        O::IntegerShift {
            psi_operation,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { obligation },
            direction: IntegerShiftDirection::Right,
            count,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::ExactShiftCount {
                value_type: *value_type,
                count_type: *count_type,
                count: value_term(*count, *count_type),
            },
        )),
        O::IntegerArithmetic {
            psi_operation,
            scalar_type,
            arithmetic,
            left,
            right,
            ..
        } if arithmetic.operator().is_division() => {
            let obligation = arithmetic.obligation()?;
            // An exact quotient also excludes the signed `MIN / -1` overflow;
            // wrapping and saturating division define that case and only
            // need a nonzero divisor.
            let goal = match arithmetic.policy() {
                IntegerOverflowPolicy::Exact => CanonicalScalarGoal::ExactDivisionDefined {
                    integer_type: *scalar_type,
                    left: value_term(*left, *scalar_type),
                    right: value_term(*right, *scalar_type),
                },
                IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Saturating => {
                    CanonicalScalarGoal::NonzeroDivisor {
                        integer_type: *scalar_type,
                        divisor: value_term(*right, *scalar_type),
                    }
                }
            };
            Some((*psi_operation, obligation, goal))
        }
        _ => None,
    }
}

/// The canonical byte spelling of an integer type or value shared by range
/// facts and rewrite encodings.
pub(crate) fn encode_integer_type(canonical: &mut Vec<u8>, scalar_type: IntegerType) {
    canonical.push(match scalar_type.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    canonical.push(match scalar_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    crate::codec_bytes::push_encoded(canonical, scalar_type.bits().to_le_bytes());
}

pub(crate) fn encode_integer_value(canonical: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            canonical.push(1);
            crate::codec_bytes::push_encoded(canonical, value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            canonical.push(2);
            crate::codec_bytes::push_encoded(canonical, value.to_le_bytes());
        }
    }
}
