//! Preserve copied scalar facts when current storage observations expire.
//!
//! A live, exactly typed `field = value` equation records the value of that
//! canonical leaf before a write. Re-express affected facts through one such
//! immutable SSA value, then apply the original invalidation predicate. This
//! preserves correlations already observed without asserting anything about
//! new storage. Leaves without a live exact scalar equality and other
//! observation kinds still expire; reads, stores and guarantees may supply it.
//!
//! Rewrite in place at invalidation, not by appending variants at every read:
//! one original fact remains at most one fact, even across many captured fields.
//! Only unconditional equality leaves supply substitutions. Conditional facts
//! retain their entire connective; a hypothesis never becomes an ambient fact.

use std::collections::BTreeSet;

use semantic_vocabulary::{Proposition, ScalarTerm};

pub(super) fn retain(
    axioms: &mut Vec<Proposition>,
    capture: bool,
    keep: impl Fn(&Proposition) -> bool,
) {
    if !capture {
        axioms.retain(keep);
        return;
    }
    if axioms.iter().all(&keep) {
        return;
    }
    // The association is sparse and local to this write. Reverse ledger order
    // chooses the latest live equality, not every possible capture of a leaf.
    let mut captures = Vec::new();
    let mut pending = axioms.iter().collect::<Vec<_>>();
    while let Some(fact) = pending.pop() {
        match fact {
            Proposition::Conjunction(members) => pending.extend(members),
            Proposition::Equal(left, right) => {
                for (field, value) in [(left, right), (right, left)] {
                    if matches!(
                        field,
                        ScalarTerm::IntegerField { .. }
                            | ScalarTerm::BooleanField { .. }
                            | ScalarTerm::IeeeFloatField { .. }
                    ) && matches!(value, ScalarTerm::Value { .. })
                        && field.scalar_type() == value.scalar_type()
                        && !keep(&Proposition::Equal(field.clone(), field.clone()))
                        && !captures.iter().any(|(existing, _)| existing == field)
                    {
                        captures.push((field.clone(), value.clone()));
                    }
                }
            }
            _ => {}
        }
    }
    axioms.retain_mut(|fact| {
        if keep(fact) {
            return true;
        }
        capture_proposition(fact, &captures);
        keep(fact)
    });
    // Capturing `field = value` into `captured = value` can restate an
    // equation the roster already carries in the opposite operand order.
    // One equation kept in two directions reads as a cyclic definition to
    // the denotation checker, so the earlier direction wins and the
    // duplicate goes.
    let mut seen = BTreeSet::new();
    axioms.retain(|fact| match fact {
        Proposition::Equal(left, right)
            if matches!(left, ScalarTerm::Value { .. })
                && matches!(right, ScalarTerm::Value { .. }) =>
        {
            seen.insert((left.min(right).clone(), left.max(right).clone()))
        }
        _ => true,
    });
}

fn capture_proposition(proposition: &mut Proposition, captures: &[(ScalarTerm, ScalarTerm)]) {
    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            capture_scalar(left, captures);
            capture_scalar(right, captures);
        }
        Proposition::ScalarIeeeFloatComparison {
            kind, left, right, ..
        } => {
            capture_scalar(left, captures);
            capture_scalar(right, captures);
            if kind.is_symmetric() && left > right {
                std::mem::swap(left, right);
            }
        }
        Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
            for member in members {
                capture_proposition(member, captures);
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            capture_proposition(premise, captures);
            capture_proposition(conclusion, captures);
        }
        // Other observation families have no scalar-field capture rule. The
        // unchanged invalidation check decides whether they survive.
        _ => {}
    }
}

fn capture_scalar(term: &mut ScalarTerm, captures: &[(ScalarTerm, ScalarTerm)]) {
    match term {
        ScalarTerm::IntegerField { .. }
        | ScalarTerm::BooleanField { .. }
        | ScalarTerm::IeeeFloatField { .. } => {
            if let Some((_, value)) = captures.iter().find(|(field, _)| field == term) {
                *term = value.clone();
            }
        }
        // View extents are not entry-snapshotted values — a requires-side
        // extent is discharged against the caller's live view at the callsite.
        // A saved Boolean result does not preserve the current byte contents.
        ScalarTerm::ByteSequencePredicate { .. } | ScalarTerm::ViewExtent { .. } => {}
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => capture_scalar(operand, captures),
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
            capture_scalar(left, captures);
            capture_scalar(right, captures);
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            capture_scalar(value, captures);
            capture_scalar(count, captures);
        }
        ScalarTerm::Value { .. }
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. }
        | ScalarTerm::IeeeFloatLiteral(_) => {}
    }
}
