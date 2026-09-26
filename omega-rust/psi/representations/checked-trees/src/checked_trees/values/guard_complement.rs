//! Whether a second authored guard holds exactly where the first fails.
//!
//! A tail `g1 -> A` then `g2 -> B` with no authored fallback lowers as one
//! conditional `if g1 then A else B` only when `g2` is `!g1` on every input.
//! The graph producers and their lowering mirrors all decide that with this
//! one predicate over checked Guard rows, so a pair that composes in one graph
//! family composes in every other. Each stage still selects and validates its
//! own rows and resolves the subject's declared cases itself.
//!
//! Re-evaluating the second guard is stable by construction: checked Boolean
//! rows contain no calls, atomic observations, or deferred match arms, and no
//! statement runs between two consecutive guards, so every read, including a
//! mutable `StorageRead`, observes the value the first guard saw.

use super::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedStructuralParameterField,
};

#[cfg(test)]
mod tests;

/// `first` and `second` are exact complements when, after `!` and equality
/// with a Boolean constant are folded into a polarity:
///
/// - they are the same atom with opposite polarity. This covers `x`/`!x`,
///   `x == true`/`x == false`, and builtin `a == b`/`a != b`, whose checked
///   form is the negated equality;
/// - they are IEEE `==` and `!=` over identical operands with equal polarity,
///   which are complements even at NaN;
/// - they test different cases of one subject with equal polarity, and
///   `declared_cases` reports that the subject's sum declares exactly those two
///   cases.
///
/// Ordered comparisons are not paired: NaN makes float `<` and `>=` both
/// false, and integer orderings are normalized to distinct atoms rather than
/// to negations.
pub fn exact_complement(
    first: &CheckedBooleanExpression,
    second: &CheckedBooleanExpression,
    declared_cases: impl FnOnce(&CheckedStructuralParameterField) -> Option<Vec<String>>,
) -> bool {
    use CheckedBooleanExpression as Boolean;
    let (first, first_polarity) = polarity(first);
    let (second, second_polarity) = polarity(second);
    if first == second {
        return first_polarity != second_polarity;
    }
    if first_polarity != second_polarity {
        return false;
    }
    match (first, second) {
        (
            Boolean::ScalarIeeeFloatComparison {
                kind: first_kind,
                left: first_left,
                right: first_right,
            },
            Boolean::ScalarIeeeFloatComparison {
                kind: second_kind,
                left: second_left,
                right: second_right,
            },
        ) => {
            ieee_equality_pair(*first_kind, *second_kind)
                && first_left == second_left
                && first_right == second_right
        }
        (
            Boolean::IeeeFloatComparison {
                kind: first_kind,
                primitive_type: first_type,
                left: first_left,
                right: first_right,
            },
            Boolean::IeeeFloatComparison {
                kind: second_kind,
                primitive_type: second_type,
                left: second_left,
                right: second_right,
            },
        ) => {
            ieee_equality_pair(*first_kind, *second_kind)
                && first_type == second_type
                && first_left == second_left
                && first_right == second_right
        }
        (
            Boolean::StructuralCaseMembership {
                subject: first_subject,
                case: first_case,
            },
            Boolean::StructuralCaseMembership {
                subject: second_subject,
                case: second_case,
            },
        ) => {
            first_subject == second_subject
                && first_case != second_case
                && declared_cases(first_subject).is_some_and(|cases| {
                    cases.len() == 2 && cases.contains(first_case) && cases.contains(second_case)
                })
        }
        _ => false,
    }
}

fn ieee_equality_pair(
    first: CheckedIeeeFloatComparisonKind,
    second: CheckedIeeeFloatComparisonKind,
) -> bool {
    use CheckedIeeeFloatComparisonKind::{Equal, NotEqual};
    matches!((first, second), (Equal, NotEqual) | (NotEqual, Equal))
}

/// Fold `!` and equality with a Boolean constant into a polarity over the
/// remaining atom: `!(x == false)` is `x` with positive polarity.
fn polarity(mut expression: &CheckedBooleanExpression) -> (&CheckedBooleanExpression, bool) {
    use CheckedBooleanExpression as Boolean;
    let mut polarity = true;
    loop {
        match expression {
            Boolean::Not(operand) => {
                expression = operand;
                polarity = !polarity;
            }
            Boolean::Equal { left, right } => match (left.as_ref(), right.as_ref()) {
                (Boolean::Constant(value), operand) | (operand, Boolean::Constant(value)) => {
                    expression = operand;
                    polarity = polarity == *value;
                }
                _ => return (expression, polarity),
            },
            _ => return (expression, polarity),
        }
    }
}
