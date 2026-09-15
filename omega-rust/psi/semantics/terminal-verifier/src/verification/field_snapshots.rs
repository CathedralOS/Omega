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
                        ScalarTerm::IntegerField { .. } | ScalarTerm::BooleanField { .. }
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
}

fn capture_proposition(proposition: &mut Proposition, captures: &[(ScalarTerm, ScalarTerm)]) {
    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            capture_scalar(left, captures);
            capture_scalar(right, captures);
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
        ScalarTerm::IntegerField { .. } | ScalarTerm::BooleanField { .. } => {
            if let Some((_, value)) = captures.iter().find(|(field, _)| field == term) {
                *term = value.clone();
            }
        }
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
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{Proposition, ScalarTerm, capture_proposition, retain};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, PlaceId, ScalarType, StructuralFieldId, ValueId,
    };

    fn field(root: u64, leaf: u64) -> ScalarTerm {
        ScalarTerm::integer_field_path(
            PlaceId::new(root).unwrap(),
            vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(leaf).unwrap(),
            )],
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        )
    }

    fn value(identity: u64, field: &ScalarTerm) -> ScalarTerm {
        ScalarTerm::value(ValueId::new(identity).unwrap(), field.scalar_type())
    }

    fn forget_root(axioms: &mut Vec<Proposition>, root: u64, capture: bool) {
        retain(axioms, capture, |fact| {
            !crate::validation::proposition_observes_places(fact, &[PlaceId::new(root).unwrap()])
        });
    }

    #[test]
    fn captures_compound_correlations_without_promoting_conditional_facts() {
        let first = field(1, 1);
        let second = field(2, 1);
        let first_value = value(1, &first);
        let second_value = value(2, &second);
        let original = Proposition::Implication {
            premise: Box::new(Proposition::LessThan(first.clone(), second.clone())),
            conclusion: Box::new(Proposition::Disjunction(vec![
                Proposition::Equal(first.clone(), second.clone()),
                Proposition::Falsehood,
            ])),
        };
        let mut facts = vec![
            original.clone(),
            Proposition::Conjunction(vec![
                Proposition::Equal(first_value.clone(), first.clone()),
                Proposition::Equal(second.clone(), second_value.clone()),
            ]),
        ];
        forget_root(&mut facts, 1, true);
        forget_root(&mut facts, 2, true);
        let mut expected = original;
        capture_proposition(
            &mut expected,
            &[(first, first_value), (second, second_value)],
        );
        assert_eq!(facts.len(), 2, "no fact variants are appended");
        assert_eq!(facts[0], expected, "conditional structure survives intact");
    }

    #[test]
    fn missing_conditional_or_wrong_identity_equations_cannot_preserve_storage() {
        let observed = field(1, 1);
        let saved = value(1, &observed);
        let equation = Proposition::Equal(observed.clone(), saved.clone());
        let goal = Proposition::LessOrEqual(observed.clone(), observed.clone());
        for unusable in [
            Proposition::Implication {
                premise: Box::new(Proposition::Truth),
                conclusion: Box::new(equation.clone()),
            },
            Proposition::Disjunction(vec![equation.clone(), Proposition::Truth]),
            Proposition::Equal(field(2, 1), saved.clone()),
            Proposition::Equal(field(1, 2), saved.clone()),
            Proposition::Equal(
                observed.clone(),
                ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean),
            ),
        ] {
            let mut facts = vec![goal.clone(), unusable];
            forget_root(&mut facts, 1, true);
            assert!(!facts.contains(&goal));
            assert!(
                !facts
                    .iter()
                    .any(|fact| matches!(fact, Proposition::LessOrEqual(..)))
            );
        }
        let mut private_crash = vec![goal, equation];
        forget_root(&mut private_crash, 1, false);
        assert!(
            private_crash.is_empty(),
            "private crash reconstruction does not capture current facts"
        );
    }

    #[test]
    fn a_later_read_cannot_restore_correlation_destroyed_before_capture() {
        let first = field(1, 1);
        let second = field(2, 1);
        let first_value = value(1, &first);
        let second_value = value(2, &second);
        let mut facts = vec![
            Proposition::LessThan(first.clone(), second.clone()),
            Proposition::Equal(first.clone(), first_value),
        ];
        forget_root(&mut facts, 2, true);
        facts.push(Proposition::Equal(second, second_value));
        forget_root(&mut facts, 1, true);
        forget_root(&mut facts, 2, true);
        assert!(
            !facts
                .iter()
                .any(|fact| matches!(fact, Proposition::LessThan(..)))
        );
    }

    #[test]
    fn exact_written_path_preserves_siblings_and_snapshot_count_stays_linear() {
        let root = PlaceId::new(1).unwrap();
        let first = field(1, 1);
        let sibling = field(1, 2);
        let saved = value(1, &first);
        let relation = Proposition::LessThan(first.clone(), sibling.clone());
        let sibling_fact = Proposition::Equal(sibling.clone(), value(2, &sibling));
        let mut facts = vec![
            relation,
            Proposition::Equal(first.clone(), saved.clone()),
            sibling_fact.clone(),
        ];
        let ScalarTerm::IntegerField { path, .. } = &first else {
            unreachable!()
        };
        retain(&mut facts, true, |fact| {
            !crate::validation::proposition_observes_write(fact, root, path)
        });
        assert_eq!(facts[0], Proposition::LessThan(saved, sibling));
        assert!(facts.contains(&sibling_fact));
        assert_eq!(facts.len(), 3);
        for identity in 3..35 {
            facts.push(Proposition::Equal(first.clone(), value(identity, &first)));
            let before = facts.len();
            retain(&mut facts, true, |fact| {
                !crate::validation::proposition_observes_write(fact, root, path)
            });
            assert_eq!(facts.len(), before);
        }
    }
}
