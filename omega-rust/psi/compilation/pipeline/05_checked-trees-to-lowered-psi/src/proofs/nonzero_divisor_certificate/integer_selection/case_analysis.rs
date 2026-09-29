//! Compose ordinary integer certificates under each retained alternative.

use super::super::integer_evidence::ProjectedFact;
use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

mod dependencies;

/// The implications a goal can reach through value dependencies, for the
/// implication search that composes them.
pub(super) fn connected_implications<'a>(
    goal: &Proposition,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    dependencies::connected_implications(goal, assumptions, semantic_axioms)
}

/// Prove a goal the caller has already failed to prove under `assumptions`
/// by eliminating each connected case; only a branch that adds an
/// alternative retries the ordinary builder.
pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut ordinary: impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    // Stable citation order prevents revisiting permutations of the same
    // cases. Each recursive path consumes a strict suffix of this finite set.
    let cases = dependencies::connected_cases(goal, assumptions, semantic_axioms);
    eliminate_cases(goal, assumptions, semantic_axioms, &cases, &mut ordinary)
}

fn prove_branch(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    cases: &[ProjectedFact<'_>],
    ordinary: &mut impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    if let Some(proof) = ordinary(assumptions) {
        return Some(proof);
    }
    eliminate_cases(goal, assumptions, semantic_axioms, cases, ordinary)
}

fn eliminate_cases(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    cases: &[ProjectedFact<'_>],
    ordinary: &mut impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let fact = cases.first()?;
    let Proposition::Disjunction(disjuncts) = fact.proposition else {
        unreachable!("only retained disjunctions become cases")
    };
    // Implication search can re-enter the ordinary case producer inside an
    // already-selected branch. Splitting that same disjunction again only
    // appends duplicate assumptions, defeats scope memoization and multiplies
    // the search. The caller already tried this scope: skip a resolved case
    // without another leaf attempt, but retain genuinely new implied cases.
    // Only unconditional conjunction descent exposes facts; a conditional
    // conclusion or another unselected disjunct supplies no such evidence.
    if disjuncts.iter().any(|disjunct| {
        assumptions
            .iter()
            .chain(semantic_axioms)
            .any(|fact| contains_unconditional(fact, disjunct))
    }) {
        return eliminate_cases(goal, assumptions, semantic_axioms, &cases[1..], ordinary);
    }
    let branches = disjuncts
        .iter()
        .map(|disjunct| {
            let mut branch_assumptions = assumptions.to_vec();
            branch_assumptions.push(disjunct.clone());
            // The assumed alternative can carry alternatives of its own
            // (a nested computation's negative polarity, for example).
            // They are facts only inside this branch, so they extend this
            // branch's roster rather than the ambient case list; the
            // remaining shared cases still keep their citation order.
            let mut branch_cases = cases[1..].to_vec();
            for nested in super::super::integer_evidence::nested_case_facts(
                assumptions.len(),
                branch_assumptions
                    .last()
                    .expect("branch assumption pushed above"),
            ) {
                if !branch_cases
                    .iter()
                    .any(|case| case.proposition == nested.proposition)
                {
                    branch_cases.push(nested);
                }
            }
            prove_branch(
                goal,
                &branch_assumptions,
                semantic_axioms,
                &branch_cases,
                ordinary,
            )
        })
        .collect::<Option<Vec<_>>>();
    if let Some(branches) = branches {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::DisjunctionElimination {
                disjunction: Box::new(fact.proof()),
                branches,
            },
        });
    }
    // Splitting on this case first loses nothing: a proof that splits
    // on a later case first is still available inside each of this
    // case's branches, whose extra assumption the monotone builder
    // tolerates, and one of those branches already failed. So every later
    // case fails too; trying them costs 2^n builder searches and never
    // changes the answer.
    None
}

fn contains_unconditional(fact: &Proposition, wanted: &Proposition) -> bool {
    fact == wanted
        || matches!(fact, Proposition::Conjunction(parts)
            if parts.iter().any(|part| contains_unconditional(part, wanted)))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeSet;

    use proof_admission::check_certificate;
    use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};

    use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;

    fn polarity(identity: u64, positive: bool) -> Proposition {
        Proposition::Equal(
            ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean),
            ScalarTerm::Boolean(positive),
        )
    }

    #[test]
    fn an_already_selected_case_does_not_retry_the_same_scope() {
        let selected = polarity(1, true);
        let alternative = polarity(1, false);
        let cases = Proposition::Disjunction(vec![selected.clone(), alternative]);
        let goal = polarity(1, false);
        for in_axioms in [false, true] {
            let established = Proposition::Conjunction(vec![selected.clone()]);
            let assumptions = if in_axioms {
                vec![]
            } else {
                vec![established.clone()]
            };
            let mut axioms = vec![cases.clone()];
            if in_axioms {
                axioms.push(established);
            }
            let attempts = Cell::new(0);
            assert!(
                super::prove(&goal, &assumptions, &axioms, |_| {
                    attempts.set(attempts.get() + 1);
                    None
                })
                .is_none()
            );
            assert_eq!(
                attempts.get(),
                0,
                "an established alternative adds no new evidence"
            );
        }

        // A conditional alternative is not an established one. The ordinary
        // leaf has already failed in the ambient scope, but must try a branch.
        let conditional = Proposition::Implication {
            premise: Box::new(polarity(2, true)),
            conclusion: Box::new(selected),
        };
        let attempts = Cell::new(0);
        assert!(
            super::prove(&goal, &[conditional], &[cases], |_| {
                attempts.set(attempts.get() + 1);
                None
            })
            .is_none()
        );
        assert_eq!(attempts.get(), 1);
    }

    #[test]
    fn an_implication_can_introduce_new_cases_inside_a_selected_branch() {
        let context = PropositionContext::from_value_types(
            (1..=3).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean)),
        )
        .unwrap();
        let first = polarity(1, true);
        let second = polarity(1, false);
        let nested_first = polarity(2, true);
        let nested_second = polarity(2, false);
        let goal = polarity(3, true);
        let implication = |premise, conclusion| Proposition::Implication {
            premise: Box::new(premise),
            conclusion: Box::new(conclusion),
        };
        let mut axioms = vec![
            Proposition::Disjunction(vec![first.clone(), second.clone()]),
            implication(
                first,
                Proposition::Disjunction(vec![
                    Proposition::Conjunction(vec![goal.clone(), nested_first]),
                    Proposition::Conjunction(vec![goal.clone(), nested_second.clone()]),
                ]),
            ),
            implication(second, goal.clone()),
        ];
        let proof = produce_checked_canonical_integer_proof(
            &context,
            &goal,
            &[],
            &axioms,
            &BTreeSet::new(),
        )
        .expect("both original and newly implied alternatives establish the goal");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        let Proposition::Implication { conclusion, .. } = &mut axioms[1] else {
            unreachable!()
        };
        let Proposition::Disjunction(alternatives) = conclusion.as_mut() else {
            unreachable!()
        };
        alternatives[1] = nested_second;
        assert!(check_certificate(&context, &goal, &[], &axioms, &proof).is_err());
        assert!(
            produce_checked_canonical_integer_proof(
                &context,
                &goal,
                &[],
                &axioms,
                &BTreeSet::new(),
            )
            .is_none(),
            "the missing nested alternative cannot escape its branch"
        );
    }
}
