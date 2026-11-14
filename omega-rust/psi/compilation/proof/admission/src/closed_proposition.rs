/*
    Closed crash-guard evaluation. This is not predicate-denotation normalization.

    Build-time admission (`exclude_failure_routes`) and the interpreter (a
    boundary crash guard, and `InputObservations::evaluate`) ask whether one
    published route is true, false, or still unknown. The Crash section of
    `wiki/spec/terminal-psi/calls_and_outcomes.md` removes a route only when
    that route is false. An unknown sibling does not hide a deciding child,
    and an unknown premise with a true conclusion is true.

    `wiki/spec/terminal-psi/fact_validity.md` leaves IEEE relations unchanged
    in the bounded predicate-denotation conversion. `normalize`,
    `check_predicate_denotations`, and `verify_obligation` do not call this
    module. A float literal is decided here only while evaluating a crash
    guard, and only when both operands carry that comparison's declared IEEE
    format. A binary32 literal beside a binary64 literal stays unknown.
    Formation rejects that mismatch before admission calls this walk.

    The walk lives in this crate because both callers already depend on it and
    the decision needs `ClosedIntegerEvaluator`, which this crate owns.
    `semantic-vocabulary` cannot take that evaluator without a dependency
    cycle.
*/

use numerics::float_semantics::{FloatMeaning, FloatSemantics};
use semantic_vocabulary::{
    IeeeFloatComparisonKind, IeeeFloatValue, Proposition, ScalarTerm, ScalarType,
};

use crate::integer_rules::closed_integer::{ClosedIntegerEvaluationError, ClosedIntegerEvaluator};

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
    closed_guard_with(proposition, |leaf| closed_leaf(leaf, integers))
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
) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
    Ok(match proposition {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            let (ScalarTerm::IeeeFloatLiteral(left), ScalarTerm::IeeeFloatLiteral(right)) =
                (left, right)
            else {
                return Ok(None);
            };
            // A declared format is part of the relation. Mixed binary32 and
            // binary64 operands are not a comparison the Crash section decides.
            if left.format() != *format || right.format() != *format {
                return Ok(None);
            }
            let meaning = |value: IeeeFloatValue| match value {
                IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
                IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
            };
            let (left, right) = (meaning(*left), meaning(*right));
            Some(match kind {
                IeeeFloatComparisonKind::Equal => FloatSemantics::equal(&left, &right),
                IeeeFloatComparisonKind::NotEqual => FloatSemantics::not_equal(&left, &right),
                IeeeFloatComparisonKind::LessThan => FloatSemantics::less(&left, &right),
                IeeeFloatComparisonKind::LessOrEqual => {
                    FloatSemantics::less_or_equal(&left, &right)
                }
                IeeeFloatComparisonKind::NotLessThan => !FloatSemantics::less(&left, &right),
                IeeeFloatComparisonKind::NotLessOrEqual => {
                    !FloatSemantics::less_or_equal(&left, &right)
                }
            })
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
    use crate::check_predicate_denotations;
    use crate::integer_rules::closed_integer::ClosedIntegerEvaluator;
    use semantic_vocabulary::{
        IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue, PropositionContext, ValueId,
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

    #[test]
    fn predicate_denotation_leaves_an_open_ieee_relation_unchanged() {
        // The denotation conversion is a different owner. An IEEE relation it
        // does not rewrite stays that relation, which is the fact-validity rule
        // for this conversion. This walk is not on that path.
        let value = ValueId::new(1).unwrap();
        let subject = ScalarTerm::value(value, ScalarType::IeeeFloat(IeeeFloatFormat::Binary32));
        let goal = Proposition::ScalarIeeeFloatComparison {
            kind: IeeeFloatComparisonKind::Equal,
            format: IeeeFloatFormat::Binary32,
            left: subject.clone(),
            right: subject,
        };
        let context = PropositionContext::from_value_types([(
            value,
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        )])
        .unwrap();
        assert_eq!(
            check_predicate_denotations(&context, &goal, &[], &[])
                .unwrap()
                .goal(),
            &goal
        );
    }
}
