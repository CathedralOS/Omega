//! Substitute established structural observations without changing proposition logic.
//!
//! The caller owns the exact equality or capture premise for each replacement,
//! including its type, source, and observation epoch. The callback supplies no
//! proof authority: this traversal neither proves predicates nor makes a
//! conditional fact ambient. Unselected leaves and atomic proposition families
//! remain unchanged, and replacement terms are installed without revisiting them.

use semantic_vocabulary::{Proposition, ScalarTerm};

pub(super) fn substitute(
    proposition: &mut Proposition,
    replacement: &mut impl FnMut(&ScalarTerm) -> Option<ScalarTerm>,
) {
    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            substitute_scalar(left, replacement);
            substitute_scalar(right, replacement);
        }
        Proposition::ScalarIeeeFloatComparison {
            kind, left, right, ..
        } => {
            substitute_scalar(left, replacement);
            substitute_scalar(right, replacement);
            if kind.is_symmetric() && left > right {
                std::mem::swap(left, right);
            }
        }
        Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
            for member in members {
                substitute(member, replacement);
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            substitute(premise, replacement);
            substitute(conclusion, replacement);
        }
        // Atomic propositions outside scalar observations retain their own
        // vocabulary and are not rewritten by this traversal.
        _ => {}
    }
}

fn substitute_scalar(
    term: &mut ScalarTerm,
    replacement: &mut impl FnMut(&ScalarTerm) -> Option<ScalarTerm>,
) {
    match term {
        ScalarTerm::IntegerField { .. }
        | ScalarTerm::BooleanField { .. }
        | ScalarTerm::IeeeFloatField { .. }
        | ScalarTerm::ByteSequencePredicate { .. }
        | ScalarTerm::ViewExtent { .. } => {
            if let Some(value) = replacement(term) {
                *term = value;
            }
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => substitute_scalar(operand, replacement),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::WrappingIntegerDivide { left, right, .. }
        | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
        | ScalarTerm::SaturatingIntegerRemainder { left, right, .. } => {
            substitute_scalar(left, replacement);
            substitute_scalar(right, replacement);
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            substitute_scalar(value, replacement);
            substitute_scalar(count, replacement);
        }
        ScalarTerm::Value { .. }
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. }
        | ScalarTerm::IeeeFloatLiteral(_) => {}
    }
}
