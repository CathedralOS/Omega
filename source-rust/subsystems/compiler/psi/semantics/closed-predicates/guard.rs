//! A published crash route is ruled out only when its guard evaluates to false.
//! Short-circuit evaluation preserves a deciding child alongside unknown operands.
//! Build evaluation and the interpreter share this decision.

use numerics::float_semantics::FloatMeaning;
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

use crate::{ClosedIntegerEvaluationError, ClosedIntegerEvaluator};

/// Decide `proposition`.
///
/// `leaf` is called only for a proposition that is not a conjunction,
/// disjunction, or implication. A deciding child ends the walk, so a later
/// sibling is not visited.
pub fn closed_guard_with<E>(
    proposition: &Proposition,
    mut leaf: impl FnMut(&Proposition) -> Result<Option<bool>, E>,
) -> Result<Option<bool>, E> {
    walk(proposition, &mut leaf)
}

/// Decide a closed proposition from literals and closed integer terms.
pub fn closed_guard(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
    closed_guard_with_float_meanings(proposition, integers, &|_| None)
}

/// A float observation can have a mathematical meaning without established
/// representation bits. Only IEEE comparisons and meaning projections consult it.
pub fn closed_guard_with_float_meanings(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
    observe: &impl Fn(&ScalarTerm) -> Option<FloatMeaning>,
) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
    closed_guard_with(proposition, |leaf| closed_leaf(leaf, integers, observe))
}

fn walk<E>(
    proposition: &Proposition,
    leaf: &mut impl FnMut(&Proposition) -> Result<Option<bool>, E>,
) -> Result<Option<bool>, E> {
    match proposition {
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let identity = matches!(proposition, Proposition::Conjunction(_));
            let mut unknown = false;
            for child in children {
                match walk(child, leaf)? {
                    Some(value) if value != identity => return Ok(Some(value)),
                    Some(_) => {}
                    None => unknown = true,
                }
            }
            Ok((!unknown).then_some(identity))
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => match walk(premise, leaf)? {
            Some(false) => Ok(Some(true)),
            Some(true) => walk(conclusion, leaf),
            None => Ok(walk(conclusion, leaf)?.filter(|value| *value)),
        },
        other => leaf(other),
    }
}

fn closed_leaf(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
    observe: &impl Fn(&ScalarTerm) -> Option<FloatMeaning>,
) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
    Ok(match proposition {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
        Proposition::DeclarationIdentity(clause) if clause.is_float_meaning() => {
            crate::check_predicate_evaluation_size([proposition])
                .map_err(|_| ClosedIntegerEvaluationError::ResourceLimitExceeded)?;
            crate::ieee::closed_float_meaning_equality(clause, observe)
        }
        Proposition::ScalarIeeeFloatComparison { .. } => {
            crate::ieee::ieee_relation(proposition, observe)
        }
        Proposition::Equal(left, right) if left.scalar_type() == ScalarType::Boolean => left
            .boolean_value()
            .zip(right.boolean_value())
            .map(|(left, right)| left == right),
        semantic_vocabulary::ordered_comparison_operands!(left, right) => left
            .integer_value()
            .zip(right.integer_value())
            .and_then(|((left_type, left), (right_type, right))| {
                (left_type == right_type)
                    .then(|| left_type.compare(left, right))
                    .flatten()
            })
            .map(|ordering| match proposition {
                Proposition::Equal(..) => ordering.is_eq(),
                Proposition::LessThan(..) => ordering.is_lt(),
                _ => !ordering.is_gt(),
            }),
        semantic_vocabulary::integer_math_comparison_operands!(left, right) => integers
            .compare(left, right)?
            .map(|ordering| match proposition {
                Proposition::IntegerMathEqual(..) => ordering.is_eq(),
                Proposition::IntegerMathLessThan(..) => ordering.is_lt(),
                _ => !ordering.is_gt(),
            }),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue, ScalarTerm,
    };

    fn literal(value: IeeeFloatValue) -> ScalarTerm {
        ScalarTerm::IeeeFloatLiteral(value)
    }

    #[test]
    fn mismatched_float_formats_stay_unknown() {
        let mut integers = ClosedIntegerEvaluator::default();
        let guard = Proposition::ScalarIeeeFloatComparison {
            kind: IeeeFloatComparisonKind::LessThan,
            format: IeeeFloatFormat::Binary32,
            left: literal(IeeeFloatValue::Binary32(0x3f80_0000)),
            right: literal(IeeeFloatValue::Binary64(0x4000_0000_0000_0000)),
        };
        assert_eq!(closed_guard(&guard, &mut integers).unwrap(), None);
    }
}
