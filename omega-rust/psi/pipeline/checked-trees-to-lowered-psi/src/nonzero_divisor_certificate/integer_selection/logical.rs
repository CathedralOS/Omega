//! Canonical compound integer proposition proof construction.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

/// A selected Boolean edge can be impossible under declared call guarantees.
/// Keep that edge in the arrival roster and prove its contradiction using exact
/// citations. `false and goal` has the same false denotation; the existing
/// conversion and conjunction elimination close the goal without a new rule
/// or a trusted prune. Every contradictory citation remains in the proof.
pub(super) fn prove_boolean_contradiction(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let contradiction = super::exact::prove(
        &Proposition::Equal(ScalarTerm::Boolean(false), ScalarTerm::Boolean(true)),
        assumptions,
        semantic_axioms,
    )?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionElimination {
            conjunction: Box::new(ProofNode {
                conclusion: Proposition::Conjunction(vec![Proposition::Falsehood, goal.clone()]),
                rule: ProofRule::PredicateDenotation {
                    premise: Box::new(contradiction),
                },
            }),
            conjunct: 1,
        },
    })
}

pub(super) fn prove_conjunction(
    goal: &Proposition,
    conjuncts: &[Proposition],
    mut prove: impl FnMut(&Proposition) -> Option<ProofNode>,
) -> Option<ProofNode> {
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionIntroduction(
            conjuncts
                .iter()
                .map(&mut prove)
                .collect::<Option<Vec<_>>>()?,
        ),
    })
}

pub(super) fn prove_disjunction(
    goal: &Proposition,
    disjuncts: &[Proposition],
    mut prove: impl FnMut(&Proposition) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let (index, disjunct) = disjuncts
        .iter()
        .enumerate()
        .find_map(|(index, disjunct)| prove(disjunct).map(|proof| (index, proof)))?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(disjunct),
            index,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::check_certificate;
    use semantic_vocabulary::{PropositionContext, ScalarType, ValueId};

    #[test]
    fn impossible_boolean_arrival_retains_every_contradictory_citation() {
        let value =
            |ordinal| ScalarTerm::value(ValueId::new(ordinal).unwrap(), ScalarType::Boolean);
        let context = PropositionContext::from_value_types(
            (1..=3).map(|ordinal| (ValueId::new(ordinal).unwrap(), ScalarType::Boolean)),
        )
        .unwrap();
        let goal = Proposition::Equal(value(3), ScalarTerm::Boolean(true));
        let axioms = [
            Proposition::Equal(value(1), ScalarTerm::Boolean(false)),
            Proposition::Equal(value(1), value(2)),
            Proposition::Equal(value(2), ScalarTerm::Boolean(true)),
        ];
        let proof = prove_boolean_contradiction(&goal, &[], &axioms).unwrap();
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        for missing in 0..axioms.len() {
            let mut changed = axioms.to_vec();
            changed.remove(missing);
            assert!(check_certificate(&context, &goal, &[], &changed, &proof).is_err());
            assert!(prove_boolean_contradiction(&goal, &[], &changed).is_none());
        }
        let mut consistent = axioms.clone();
        consistent[2] = Proposition::Equal(value(2), ScalarTerm::Boolean(false));
        assert!(prove_boolean_contradiction(&goal, &[], &consistent).is_none());
        assert!(check_certificate(&context, &goal, &[], &consistent, &proof).is_err());
        // An alternative is not an unconditional contradiction.
        let alternatives = [Proposition::Disjunction(vec![
            Proposition::Conjunction(axioms.to_vec()),
            Proposition::Truth,
        ])];
        assert!(prove_boolean_contradiction(&goal, &[], &alternatives).is_none());
    }
}
