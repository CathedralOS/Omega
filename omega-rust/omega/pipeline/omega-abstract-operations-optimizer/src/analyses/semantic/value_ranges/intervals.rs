//! Closed conjunctive integer-interval extraction from canonical propositions.

use std::collections::BTreeMap;

use psi_core::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
    ValueId,
};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PartialIntegerBounds {
    pub(super) lower: Option<IntegerValue>,
    pub(super) upper: Option<IntegerValue>,
}

pub(super) enum IntervalExtraction {
    Unsupported,
    Contradiction,
    Bounds(BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>),
}

pub(super) fn extract(proposition: &Proposition) -> IntervalExtraction {
    if proposition.validate().is_err() {
        return IntervalExtraction::Unsupported;
    }
    let mut bounds = BTreeMap::new();
    if !extract_conjunct(proposition, &mut bounds) {
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

fn extract_conjunct(
    proposition: &Proposition,
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
) -> bool {
    match proposition {
        Proposition::Truth => true,
        Proposition::Falsehood => false,
        Proposition::Conjunction(conjuncts) => conjuncts
            .iter()
            .all(|conjunct| extract_conjunct(conjunct, bounds)),
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
        | Proposition::ByteSequenceEqual { .. }
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

fn integer_value_cmp(
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
    let one = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    scalar_type.exact_sub(value, one)
}

fn successor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    scalar_type.exact_add(value, one)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::PropositionId;

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
        IntegerType::new(sign, bits).expect("fixed test integer")
    }

    fn value(raw: u64, scalar_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(id(raw, ValueId::new), ScalarType::Integer(scalar_type))
    }

    fn literal(scalar_type: IntegerType, value: IntegerValue) -> ScalarTerm {
        ScalarTerm::integer(scalar_type, value).expect("admitted test literal")
    }

    #[test]
    fn interval_extraction_is_closed_conjunctive_and_nonconvex_safe() {
        let signed = integer(IntegerSign::Signed, 8);
        let count = id(1, ValueId::new);
        let proposition = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(signed, IntegerValue::Signed(0)), value(1, signed)),
            Proposition::LessOrEqual(value(1, signed), literal(signed, IntegerValue::Signed(63))),
        ]);
        let IntervalExtraction::Bounds(bounds) = extract(&proposition) else {
            panic!("signed shift bounds produce one interval")
        };
        assert_eq!(
            bounds[&(count, signed)].lower,
            Some(IntegerValue::Signed(0))
        );
        assert_eq!(
            bounds[&(count, signed)].upper,
            Some(IntegerValue::Signed(63))
        );

        let unsigned = integer(IntegerSign::Unsigned, 8);
        let divisor = id(2, ValueId::new);
        let IntervalExtraction::Bounds(bounds) = extract(&Proposition::LessOrEqual(
            literal(unsigned, IntegerValue::Unsigned(1)),
            value(2, unsigned),
        )) else {
            panic!("unsigned nonzero proof produces a lower bound")
        };
        assert_eq!(
            bounds[&(divisor, unsigned)].lower,
            Some(IntegerValue::Unsigned(1))
        );
        assert_eq!(bounds[&(divisor, unsigned)].upper, None);

        let signed_nonzero = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(value(1, signed), literal(signed, IntegerValue::Signed(-1))),
            Proposition::LessOrEqual(literal(signed, IntegerValue::Signed(1)), value(1, signed)),
        ]);
        assert!(matches!(
            extract(&signed_nonzero),
            IntervalExtraction::Unsupported
        ));
    }

    #[test]
    fn interval_extraction_rejects_empty_strict_and_conflicting_domains() {
        let unsigned = integer(IntegerSign::Unsigned, 8);
        let minimum = literal(unsigned, unsigned.minimum_value());
        let maximum = literal(unsigned, unsigned.maximum_value());
        assert!(matches!(
            extract(&Proposition::LessThan(value(1, unsigned), minimum)),
            IntervalExtraction::Contradiction
        ));
        assert!(matches!(
            extract(&Proposition::LessThan(maximum, value(1, unsigned))),
            IntervalExtraction::Contradiction
        ));
        assert!(matches!(
            extract(&Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    literal(unsigned, IntegerValue::Unsigned(1)),
                    value(1, unsigned),
                ),
                Proposition::LessOrEqual(
                    value(1, unsigned),
                    literal(unsigned, IntegerValue::Unsigned(0)),
                ),
            ])),
            IntervalExtraction::Contradiction
        ));
    }

    #[test]
    fn interval_extraction_unsupported_conjunct_keeps_an_independent_supported_bound() {
        let unsigned = integer(IntegerSign::Unsigned, 8);
        let value_id = id(1, ValueId::new);
        let proposition = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(
                literal(unsigned, IntegerValue::Unsigned(1)),
                value(1, unsigned),
            ),
            Proposition::Atom(id(9, PropositionId::new)),
        ]);
        let IntervalExtraction::Bounds(bounds) = extract(&proposition) else {
            panic!("supported conjunct remains a valid consequence")
        };
        assert_eq!(
            bounds[&(value_id, unsigned)].lower,
            Some(IntegerValue::Unsigned(1))
        );
    }
}
