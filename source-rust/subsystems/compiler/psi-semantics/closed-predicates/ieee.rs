//! Compare two literal IEEE operands in their declared format.

use numerics::float_semantics::{FloatMeaning, FloatSemantics};
use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatValue, Proposition, ScalarTerm};

pub fn closed_ieee_relation(proposition: &Proposition) -> Option<bool> {
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
