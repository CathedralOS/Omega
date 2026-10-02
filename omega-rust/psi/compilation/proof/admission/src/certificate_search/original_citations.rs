//! Restore original premise identities after searching a converted predicate view.
//! Each changed citation gains its own checked denotation or equality transport;
//! local branch assumptions remain within the original proof scope.

use crate::{ProofNode, ProofRule, check_predicate_denotations, check_value_equality_denotation};
use semantic_vocabulary::{Proposition, PropositionContext};

/// Restore citations using flat definition citations from the original semantic
/// roster. Counting this input before each clone bounds the expanded proof, not
/// merely the smaller search tree that requested the conversions. The result
/// is the remaining node budget for any enclosing conversion.
pub fn retain_original_predicate_citations(
    proof: &mut ProofNode,
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    equations: &[ProofNode],
) -> Option<usize> {
    for equation in equations {
        let ProofRule::SemanticAxiom { index } = equation.rule else {
            return None;
        };
        if semantic_axioms.get(index) != Some(&equation.conclusion) {
            return None;
        }
    }
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
                remaining = remaining.checked_sub(1)?;
                let premise = ProofNode {
                    conclusion: original.clone(),
                    rule: proof.rule.clone(),
                };
                let denotation = check_predicate_denotations(context, original, &[], &[]).ok()?;
                if denotation.goal() == &proof.conclusion {
                    proof.rule = ProofRule::PredicateDenotation {
                        premise: Box::new(premise),
                    };
                } else {
                    let transported = check_value_equality_denotation(
                        context,
                        original,
                        equations.iter().map(|proof| &proof.conclusion),
                    )
                    .ok()?;
                    if transported != proof.conclusion {
                        return None;
                    }
                    remaining = remaining.checked_sub(equations.len())?;
                    proof.rule = ProofRule::ValueEqualityTransport {
                        premise: Box::new(premise),
                        equalities: equations.to_vec(),
                    };
                }
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
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => {
                pending.push(premise);
                pending.extend(equalities);
            }
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
            | ProofRule::IntegerMathOrderTranslation {
                relation: child, ..
            }
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
            | ProofRule::IntegerAddOrder {
                sum: left,
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
            }
            | ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound: left,
                right_bound: right,
                ..
            } => {
                pending.extend([left.as_mut(), right.as_mut()]);
            }
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => {
                pending.extend([smaller.as_mut(), larger.as_mut(), order.as_mut()]);
            }
        }
    }
    Some(remaining)
}
