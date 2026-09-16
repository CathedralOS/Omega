//! The equality rules: symmetry and transitivity of equalities, predicate
//! denotation conversion and value-equality transport of a premise.

use super::integer_math_normalization::propositions_match_under_integer_math_normalization;
use super::{AcceptanceBuilder, AcceptedProofRule, ProofError, RuleScope};
use semantic_vocabulary::Proposition;
use terminal_psi::{ProofNode, ProofRule};

pub(super) fn check_equality_symmetry(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::EqualitySymmetry { equality } = &proof.rule else {
        unreachable!("dispatched check_equality_symmetry")
    };
    acceptance.rules.insert(AcceptedProofRule::EqualitySymmetry);
    let Proposition::Equal(left, right) = &equality.conclusion else {
        return Err(ProofError::RulePremiseMismatch("equality symmetry"));
    };
    (proof.conclusion == Proposition::Equal(right.clone(), left.clone()))
        .then_some(())
        .ok_or(ProofError::EqualityConclusionMismatch)
}

pub(super) fn check_predicate_denotation(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::PredicateDenotation { premise } = &proof.rule else {
        unreachable!("dispatched check_predicate_denotation")
    };
    let RuleScope { context, .. } = *scope;
    // The child is checked under the original, unchanged premise
    // roster by ordinary traversal. Conversion licenses only this one
    // conclusion, never a rewritten assumption or a new SSA equation.
    let original = crate::check_predicate_denotations(context, &premise.conclusion, &[], &[])
        .map_err(|error| ProofError::PredicateDenotation(Box::new(error)))?;
    let converted = crate::check_predicate_denotations(context, &proof.conclusion, &[], &[])
        .map_err(|error| ProofError::PredicateDenotation(Box::new(error)))?;
    if original.goal() != converted.goal() {
        return Err(ProofError::RuleConclusionMismatch("predicate denotation"));
    }
    acceptance
        .rules
        .insert(AcceptedProofRule::PredicateDenotation);
    Ok(())
}

pub(super) fn check_value_equality_transport(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::ValueEqualityTransport {
        premise,
        equalities,
    } = &proof.rule
    else {
        unreachable!("dispatched check_value_equality_transport")
    };
    let RuleScope { context, .. } = *scope;
    // Ordinary scoped traversal has checked every child. Only those
    // proved equations license transport; the ambient premise roster
    // and its citation identities remain completely unchanged.
    let equations = || equalities.iter().map(|equality| &equality.conclusion);
    let original =
        crate::check_value_equality_denotation(context, &premise.conclusion, equations())
            .map_err(|error| ProofError::PredicateDenotation(Box::new(error)))?;
    let transported =
        crate::check_value_equality_denotation(context, &proof.conclusion, equations())
            .map_err(|error| ProofError::PredicateDenotation(Box::new(error)))?;
    if original != transported {
        return Err(ProofError::RuleConclusionMismatch(
            "value equality transport",
        ));
    }
    acceptance
        .rules
        .insert(AcceptedProofRule::ValueEqualityTransport);
    Ok(())
}

pub(super) fn check_equality_transitivity(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::EqualityTransitivity {
        left_equals_middle,
        middle_equals_right,
    } = &proof.rule
    else {
        unreachable!("dispatched check_equality_transitivity")
    };
    acceptance
        .rules
        .insert(AcceptedProofRule::EqualityTransitivity);
    match (
        &left_equals_middle.conclusion,
        &middle_equals_right.conclusion,
    ) {
        (Proposition::Equal(left, first_middle), Proposition::Equal(second_middle, right)) => {
            if first_middle != second_middle {
                return Err(ProofError::EqualityMiddleMismatch);
            }
            let composed = Proposition::Equal(left.clone(), right.clone());
            if !propositions_match_under_integer_math_normalization(&composed, &proof.conclusion) {
                return Err(ProofError::EqualityConclusionMismatch);
            }
            Ok(())
        }
        (
            Proposition::IntegerMathEqual(left, first_middle),
            Proposition::IntegerMathEqual(second_middle, right),
        ) => {
            if first_middle != second_middle {
                return Err(ProofError::EqualityMiddleMismatch);
            }
            let mut left = left.clone();
            let mut right = right.clone();
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            (proof.conclusion == Proposition::IntegerMathEqual(left, right))
                .then_some(())
                .ok_or(ProofError::EqualityConclusionMismatch)
        }
        (
            Proposition::ContentConservation(left_equation),
            Proposition::ContentConservation(right_equation),
        ) => {
            let Proposition::ContentConservation(expected) = &proof.conclusion else {
                return Err(ProofError::RuleConclusionMismatch("equality transitivity"));
            };
            if left_equation.algebra() != right_equation.algebra()
                || left_equation.algebra() != expected.algebra()
            {
                return Err(ProofError::EqualityAlgebraMismatch);
            }
            let left_terms = [left_equation.left(), left_equation.right()];
            let right_terms = [right_equation.left(), right_equation.right()];
            let mut shared_middle = false;
            for (left_index, left_term) in left_terms.iter().enumerate() {
                for (right_index, right_term) in right_terms.iter().enumerate() {
                    if left_term != right_term {
                        continue;
                    }
                    shared_middle = true;
                    let composed = semantic_vocabulary::ContentConservation::new(
                        left_equation.algebra().clone(),
                        left_terms[1 - left_index].clone(),
                        right_terms[1 - right_index].clone(),
                    );
                    if &composed == expected {
                        return Ok(());
                    }
                }
            }
            Err(if shared_middle {
                ProofError::EqualityConclusionMismatch
            } else {
                ProofError::EqualityMiddleMismatch
            })
        }
        _ => Err(ProofError::RulePremiseMismatch("equality transitivity")),
    }
}
