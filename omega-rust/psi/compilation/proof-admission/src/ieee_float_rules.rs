//! IEEE proof rules keep comparison meaning separate from stored-bit equality.
//! Closed judgments compute the selected permanent format's exact semantics.
//! Ordered chains cite actual premises and join complete subjects; NaN-inclusive
//! complements supply no positive order edge.

use numerics::float_semantics::{FloatMeaning, FloatSemantics};
use semantic_vocabulary::{
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue, Proposition, ScalarTerm, ScalarType,
};

use crate::ProofError;

#[cfg(test)]
#[path = "ieee_float_rules/tests.rs"]
mod tests;

pub(crate) fn closed_relation(proposition: &Proposition) -> Option<bool> {
    let Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    } = proposition
    else {
        return None;
    };
    let (ScalarTerm::IeeeFloatLiteral(left), ScalarTerm::IeeeFloatLiteral(right)) = (left, right)
    else {
        return None;
    };
    if left.format() != *format || right.format() != *format {
        return None;
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
        IeeeFloatComparisonKind::LessOrEqual => FloatSemantics::less_or_equal(&left, &right),
        IeeeFloatComparisonKind::NotLessThan => !FloatSemantics::less(&left, &right),
        IeeeFloatComparisonKind::NotLessOrEqual => !FloatSemantics::less_or_equal(&left, &right),
    })
}

struct Order<'a> {
    format: IeeeFloatFormat,
    left: &'a ScalarTerm,
    right: &'a ScalarTerm,
    strict: bool,
}

fn order(proposition: &Proposition) -> Option<Order<'_>> {
    let Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    } = proposition
    else {
        return None;
    };
    let strict = match kind {
        IeeeFloatComparisonKind::LessThan => true,
        IeeeFloatComparisonKind::LessOrEqual => false,
        _ => return None,
    };
    let operand = ScalarType::IeeeFloat(*format);
    (left.scalar_type() == operand && right.scalar_type() == operand).then_some(Order {
        format: *format,
        left,
        right,
        strict,
    })
}

/// Every premise is independently checked by the certificate traversal first.
/// Positive IEEE order is transitive, including infinities and equal signed
/// zeros. A strict conclusion needs a strict edge; an empty chain establishes
/// nothing, in particular no reflexive fact about a possibly-NaN value.
pub(crate) fn check_chain<'a>(
    conclusion: &Proposition,
    premises: impl IntoIterator<Item = &'a Proposition>,
) -> Result<(), ProofError> {
    let invalid = ProofError::IeeeFloatOrderMismatch;
    let goal = order(conclusion).ok_or_else(|| invalid("conclusion is not positive IEEE order"))?;
    let mut premises = premises.into_iter();
    let first = premises
        .next()
        .and_then(order)
        .ok_or_else(|| invalid("chain has no positive ordered premise"))?;
    if first.format != goal.format || first.left != goal.left {
        return Err(invalid("first premise changes the format or subject"));
    }
    let mut last = first.right;
    let mut strict = first.strict;
    for premise in premises {
        let next = order(premise).ok_or_else(|| invalid("premise is not positive IEEE order"))?;
        if next.format != goal.format || next.left != last {
            return Err(invalid("middle subject or format does not rejoin"));
        }
        last = next.right;
        strict |= next.strict;
    }
    if last != goal.right {
        return Err(invalid("last premise changes the result subject"));
    }
    if goal.strict && !strict {
        return Err(invalid("no premise establishes strict order"));
    }
    Ok(())
}
