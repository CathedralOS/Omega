//! Canonical compound integer proposition proof construction.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

/// A selected edge can be impossible under a literal guard or declared guarantees.
/// Keep that edge in the arrival roster and prove its contradiction using exact
/// citations. `false and goal` has the same false denotation; the existing
/// conversion and conjunction elimination close the goal without a new rule
/// or a trusted prune. Every contradictory citation remains in the proof.
pub(super) fn prove_contradiction(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    // Reconstruction closes a literal guard directly to Falsehood, whereas a
    // call-produced guard can retain contradictory value equations. Both need
    // exact citations: neither permits pruning the impossible arrival itself.
    let contradiction =
        super::super::integer_evidence::projected_facts(assumptions, semantic_axioms)
            .into_iter()
            .find(|fact| fact.proposition == &Proposition::Falsehood)
            .map(|fact| fact.proof())
            .or_else(|| {
                super::exact::prove(
                    &Proposition::Equal(ScalarTerm::Boolean(false), ScalarTerm::Boolean(true)),
                    assumptions,
                    semantic_axioms,
                )
            })?;
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
    fn explicit_falsehood_requires_its_exact_unconditional_citation() {
        let value = ValueId::new(1).unwrap();
        let context = PropositionContext::from_value_types([(value, ScalarType::Boolean)]).unwrap();
        let goal = Proposition::Equal(
            ScalarTerm::value(value, ScalarType::Boolean),
            ScalarTerm::Boolean(true),
        );
        let nested_falsehood = Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Conjunction(vec![Proposition::Truth, Proposition::Falsehood]),
        ]);
        for fact in [Proposition::Falsehood, nested_falsehood] {
            for is_assumption in [false, true] {
                let facts = [Proposition::Truth, fact.clone()];
                let (assumptions, axioms): (&[_], &[_]) = if is_assumption {
                    (&facts, &[])
                } else {
                    (&[], &facts)
                };
                let proof = prove_contradiction(&goal, assumptions, axioms).unwrap();
                check_certificate(&context, &goal, assumptions, axioms, &proof).unwrap();
                // Changing the cited row or moving its conjunction leaf cannot
                // repair an old proof, even if the new roster is contradictory.
                for replacement in [
                    vec![],
                    vec![fact.clone()],
                    vec![Proposition::Truth, Proposition::Truth],
                    vec![
                        Proposition::Truth,
                        Proposition::Conjunction(vec![
                            Proposition::Truth,
                            Proposition::Conjunction(vec![
                                Proposition::Falsehood,
                                Proposition::Truth,
                            ]),
                        ]),
                    ],
                ] {
                    let (assumptions, axioms): (&[_], &[_]) = if is_assumption {
                        (&replacement, &[])
                    } else {
                        (&[], &replacement)
                    };
                    assert!(
                        check_certificate(&context, &goal, assumptions, axioms, &proof).is_err()
                    );
                }
            }
        }
        for conditional in [
            Proposition::Disjunction(vec![Proposition::Falsehood, Proposition::Truth]),
            Proposition::Implication {
                premise: Box::new(goal.clone()),
                conclusion: Box::new(Proposition::Falsehood),
            },
        ] {
            assert!(prove_contradiction(&goal, std::slice::from_ref(&conditional), &[]).is_none());
            assert!(prove_contradiction(&goal, &[], &[conditional]).is_none());
        }
        assert!(prove_contradiction(&goal, &[], &[]).is_none());
    }

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
        let proof = prove_contradiction(&goal, &[], &axioms).unwrap();
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        for missing in 0..axioms.len() {
            let mut changed = axioms.to_vec();
            changed.remove(missing);
            assert!(check_certificate(&context, &goal, &[], &changed, &proof).is_err());
            assert!(prove_contradiction(&goal, &[], &changed).is_none());
        }
        let mut consistent = axioms.clone();
        consistent[2] = Proposition::Equal(value(2), ScalarTerm::Boolean(false));
        assert!(prove_contradiction(&goal, &[], &consistent).is_none());
        assert!(check_certificate(&context, &goal, &[], &consistent, &proof).is_err());
        // An alternative is not an unconditional contradiction.
        let alternatives = [Proposition::Disjunction(vec![
            Proposition::Conjunction(axioms.to_vec()),
            Proposition::Truth,
        ])];
        assert!(prove_contradiction(&goal, &[], &alternatives).is_none());
    }
}
