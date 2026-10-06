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
        super::observation_substitution::substitute(fact, &mut |field| {
            captures
                .iter()
                .find(|(captured, _)| captured == field)
                .map(|(_, value)| value.clone())
        });
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
