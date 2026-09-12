//! Search an equivalent predicate view, then make each conversion explicit.
//! Published goals and premise slots never change: normalized citations are
//! backed by original citations under a checked PredicateDenotation step.

use proof_admission::{ProofNode, ProofRule, check_predicate_denotations};
use semantic_vocabulary::{Proposition, PropositionContext, ValueId};
use std::collections::BTreeSet;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameters: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let converted =
        check_predicate_denotations(context, goal, assumptions, semantic_axioms).ok()?;
    if converted.goal() == goal
        && converted.requirements() == assumptions
        && converted.semantic_axioms() == semantic_axioms
    {
        return None;
    }
    let mut proof = super::integer_selection::build_with_machine_parameters(
        context,
        converted.goal(),
        converted.requirements(),
        converted.semantic_axioms(),
        machine_parameters,
    )?;
    retain_original_citations(&mut proof, assumptions, semantic_axioms)?;
    Some(if &proof.conclusion == goal {
        proof
    } else {
        ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(proof),
            },
        }
    })
}

fn retain_original_citations(
    proof: &mut ProofNode,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<()> {
    let mut pending = vec![proof];
    let mut remaining = 4096usize;
    while let Some(proof) = pending.pop() {
        remaining = remaining.checked_sub(1)?;
        let original = match &proof.rule {
            ProofRule::Assumption { index } => assumptions.get(*index),
            ProofRule::SemanticAxiom { index } => Some(semantic_axioms.get(*index)?),
            _ => None,
        };
        if let Some(original) = original {
            if original != &proof.conclusion {
                let premise = ProofNode {
                    conclusion: original.clone(),
                    rule: proof.rule.clone(),
                };
                proof.rule = ProofRule::PredicateDenotation {
                    premise: Box::new(premise),
                };
            }
            continue;
        }
        match &mut proof.rule {
            // Locally discharged assumptions were introduced by the converted
            // proof itself and do not refer to the ambient original roster.
            ProofRule::Assumption { .. }
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Primitive(_)
            | ProofRule::IntegerCorrelatedForbiddenRoots { .. } => {}
            ProofRule::ConjunctionIntroduction(children) => pending.extend(children),
            ProofRule::ConjunctionElimination {
                conjunction: child, ..
            }
            | ProofRule::DisjunctionIntroduction {
                disjunct: child, ..
            }
            | ProofRule::ImplicationIntroduction { body: child }
            | ProofRule::EqualitySymmetry { equality: child }
            | ProofRule::PredicateDenotation { premise: child }
            | ProofRule::IntegerOrderWeakening { relation: child }
            | ProofRule::IntegerOrderDiscreteness { relation: child }
            | ProofRule::IntegerAffineBound {
                root_bound: child, ..
            }
            | ProofRule::IntegerCastBound {
                root_bound: child, ..
            } => pending.push(child),
            ProofRule::DisjunctionElimination {
                disjunction,
                branches,
            } => {
                pending.push(disjunction);
                pending.extend(branches);
            }
            ProofRule::ImplicationElimination {
                implication: left,
                premise: right,
            }
            | ProofRule::EqualityTransitivity {
                left_equals_middle: left,
                middle_equals_right: right,
            }
            | ProofRule::IntegerSubtractOrder {
                difference: left,
                positive: right,
            }
            | ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: left,
                middle_less_or_equal_right: right,
            }
            | ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: left,
                middle_to_right: right,
            }
            | ProofRule::IntegerOrderSubstitution {
                relation: left,
                equality: right,
                ..
            }
            | ProofRule::IntegerExactAddDefinitionBound {
                left_bound: left,
                right_bound: right,
                ..
            } => {
                pending.extend([left.as_mut(), right.as_mut()]);
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::{AcceptedProofRule, accept_certificate};
    use semantic_vocabulary::{ScalarTerm, ScalarType};

    #[test]
    fn converted_search_rejoins_the_original_premise_in_its_original_slot() {
        let value = ValueId::new(1).unwrap();
        let context = PropositionContext::from_value_types([(value, ScalarType::Boolean)]).unwrap();
        let term = ScalarTerm::value(value, ScalarType::Boolean);
        let original = Proposition::Equal(
            ScalarTerm::boolean_not(term.clone()).unwrap(),
            ScalarTerm::boolean(true),
        );
        let goal = Proposition::Equal(term, ScalarTerm::boolean(false));
        for semantic in [false, true] {
            let original_roster = vec![Proposition::Truth, original.clone()];
            let (assumptions, axioms) = if semantic {
                (&[][..], original_roster.as_slice())
            } else {
                (original_roster.as_slice(), &[][..])
            };
            let proof = prove(&context, &goal, assumptions, axioms, &BTreeSet::new()).unwrap();
            let acceptance =
                accept_certificate(&context, &goal, assumptions, axioms, &proof).unwrap();
            assert!(
                acceptance
                    .rules
                    .contains(&AcceptedProofRule::PredicateDenotation)
            );
            let citations = if semantic {
                acceptance.semantic_axioms
            } else {
                acceptance.assumptions
            };
            assert_eq!(citations.len(), 1);
            assert_eq!(citations[0].index, 1);
            assert_eq!(citations[0].proposition, original);
            assert!(accept_certificate(&context, &goal, &[], &[], &proof).is_err());
        }
    }
}
