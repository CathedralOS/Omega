//! The integer order rules: subtract and add order, subtraction
//! antitonicity, discreteness, weakening,
//! transitivity of `<=` and strict chains, and substitution of one endpoint
//! through an equality.
//!
//! Each rule's premise/conclusion relation is a `pub(crate)` function on
//! bare propositions so the mathematical-core denotation re-decides the
//! exact check this module performs — one relation, two readers.

use super::integer_math_normalization::{
    lift_exact_integer_order, lower_integer_math_relation,
    propositions_match_under_integer_math_normalization,
};
use super::dispatched_rule;
use super::{AcceptanceBuilder, AcceptedProofRule, ProofError, mathematical_order, scalar_order};
use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarType};
use terminal_psi::{ProofNode, ProofRule};

pub(super) fn check_integer_subtract_order(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerSubtractOrder { difference, positive } => IntegerSubtractOrder);
    super::subtract_order::check(
        &difference.conclusion,
        &positive.conclusion,
        &proof.conclusion,
    )
}

pub(super) fn check_integer_add_order(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerAddOrder { sum, positive } => IntegerAddOrder);
    super::add_order::check(&sum.conclusion, &positive.conclusion, &proof.conclusion)
}

pub(super) fn check_integer_subtract_antitone(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerSubtractAntitone { smaller, larger, order } => IntegerSubtractAntitone);
    super::subtract_antitone::check(
        &smaller.conclusion,
        &larger.conclusion,
        &order.conclusion,
        &proof.conclusion,
    )
}

pub(super) fn check_integer_order_discreteness(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerOrderDiscreteness { relation } => IntegerOrderDiscreteness);
    super::order_discreteness::check(&relation.conclusion, &proof.conclusion)
}

/// The `IntegerOrderWeakening` premise/conclusion relation: equality or
/// strict order over one fixed integer carrier weakens to `<=` of the
/// exact endpoints, under the citation matcher's normalization.
pub(crate) fn order_weakening(
    relation: &Proposition,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    let (Proposition::Equal(left, right) | Proposition::LessThan(left, right)) = relation else {
        return Err(ProofError::RulePremiseMismatch("integer order weakening"));
    };
    if !matches!(
        left.scalar_type(),
        semantic_vocabulary::ScalarType::Integer(_)
    ) || left.scalar_type() != right.scalar_type()
    {
        return Err(ProofError::RulePremiseMismatch("integer order weakening"));
    }
    propositions_match_under_integer_math_normalization(
        &Proposition::LessOrEqual(left.clone(), right.clone()),
        conclusion,
    )
    .then_some(())
    .ok_or(ProofError::IntegerOrderConclusionMismatch)
}

pub(super) fn check_integer_order_weakening(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerOrderWeakening { relation } => IntegerOrderWeakening);
    order_weakening(&relation.conclusion, &proof.conclusion)
}

/// The `IntegerLessOrEqualTransitivity` premise/conclusion relation: an
/// exact shared middle and the composed `<=` conclusion, in either the
/// fixed or the mathematical carrier.
pub(crate) fn less_or_equal_transitivity(
    left_less_or_equal_middle: &Proposition,
    middle_less_or_equal_right: &Proposition,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    match (left_less_or_equal_middle, middle_less_or_equal_right) {
        (
            Proposition::LessOrEqual(left, first_middle),
            Proposition::LessOrEqual(second_middle, right),
        ) => {
            if first_middle != second_middle {
                return Err(ProofError::IntegerOrderMiddleMismatch);
            }
            let composed = Proposition::LessOrEqual(left.clone(), right.clone());
            if !propositions_match_under_integer_math_normalization(&composed, conclusion) {
                return Err(ProofError::IntegerOrderConclusionMismatch);
            }
            Ok(())
        }
        (
            Proposition::IntegerMathLessOrEqual(left, first_middle),
            Proposition::IntegerMathLessOrEqual(second_middle, right),
        ) => {
            if first_middle != second_middle {
                return Err(ProofError::IntegerOrderMiddleMismatch);
            }
            (conclusion == &Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()))
                .then_some(())
                .ok_or(ProofError::IntegerOrderConclusionMismatch)
        }
        _ => Err(ProofError::RulePremiseMismatch("integer <= transitivity")),
    }
}

pub(super) fn check_integer_less_or_equal_transitivity(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerLessOrEqualTransitivity { left_less_or_equal_middle, middle_less_or_equal_right } => IntegerLessOrEqualTransitivity);
    less_or_equal_transitivity(
        &left_less_or_equal_middle.conclusion,
        &middle_less_or_equal_right.conclusion,
        &proof.conclusion,
    )
}

pub(super) fn check_integer_strict_order_transitivity(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerStrictOrderTransitivity { left_to_middle, middle_to_right } => IntegerStrictOrderTransitivity);
    super::strict_order_transitivity::check(
        &left_to_middle.conclusion,
        &middle_to_right.conclusion,
        &proof.conclusion,
    )
}

pub(super) fn check_integer_order_substitution(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerOrderSubstitution { relation, equality, endpoint } => IntegerOrderSubstitution);
    endpoint_substitution(
        &relation.conclusion,
        &equality.conclusion,
        *endpoint,
        &proof.conclusion,
    )
}

/// The `IntegerOrderSubstitution` premise/conclusion relation: equality
/// replaces exactly one order endpoint; it never changes strictness.
pub(crate) fn endpoint_substitution(
    relation: &Proposition,
    equality: &Proposition,
    endpoint: usize,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    fn check_endpoints<T: PartialEq>(
        relation: (bool, &T, &T),
        equality: (&T, &T),
        endpoint: usize,
        conclusion: (bool, &T, &T),
    ) -> Result<(), ProofError> {
        let (relation_strict, relation_left, relation_right) = relation;
        let (conclusion_strict, conclusion_left, conclusion_right) = conclusion;
        if relation_strict != conclusion_strict {
            return Err(ProofError::IntegerOrderConclusionMismatch);
        }
        let (old_endpoint, new_endpoint) = match endpoint {
            0 => {
                if relation_right != conclusion_right {
                    return Err(ProofError::IntegerOrderUnchangedEndpointMismatch);
                }
                (relation_left, conclusion_left)
            }
            1 => {
                if relation_left != conclusion_left {
                    return Err(ProofError::IntegerOrderUnchangedEndpointMismatch);
                }
                (relation_right, conclusion_right)
            }
            endpoint => return Err(ProofError::UnknownIntegerOrderEndpoint(endpoint)),
        };
        ((equality.0 == old_endpoint && equality.1 == new_endpoint)
            || (equality.1 == old_endpoint && equality.0 == new_endpoint))
            .then_some(())
            .ok_or(ProofError::IntegerOrderSubstitutionMismatch)
    }

    if let Some(relation) = mathematical_order(relation) {
        let Proposition::IntegerMathEqual(left, right) = equality else {
            return Err(ProofError::RulePremiseMismatch(
                "integer order substitution equality",
            ));
        };
        // Exact finite rings and mathematical rings have the same denotation;
        // policy-selected arithmetic never crosses through this normalization.
        let lifted = lift_exact_integer_order(conclusion);
        let conclusion = mathematical_order(lifted.as_ref().unwrap_or(conclusion)).ok_or(
            ProofError::RuleConclusionMismatch("integer order substitution"),
        )?;
        return check_endpoints(relation, (left, right), endpoint, conclusion);
    }
    let relation = scalar_order(relation).ok_or(ProofError::RulePremiseMismatch(
        "integer order substitution relation",
    ))?;
    let Proposition::Equal(left, right) = equality else {
        return Err(ProofError::RulePremiseMismatch(
            "integer order substitution equality",
        ));
    };
    // Preserve the established fixed-to-mathematical conclusion projection.
    let normalized_conclusion =
        lower_integer_math_relation(conclusion).unwrap_or_else(|| conclusion.clone());
    let conclusion = scalar_order(&normalized_conclusion).ok_or(
        ProofError::RuleConclusionMismatch("integer order substitution"),
    )?;
    check_endpoints(relation, (left, right), endpoint, conclusion)
}

/// Translation in mathematical integers preserves a proved nonstrict order.
/// The same exact addend must occur at both endpoints; finite arithmetic and
/// strict order require their own established rules and are not coerced here.
pub(crate) fn math_order_translation(
    relation: &Proposition,
    addend: &IntegerMathTerm,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    let Proposition::IntegerMathLessOrEqual(left, right) = relation else {
        return Err(ProofError::RulePremiseMismatch(
            "mathematical integer order translation",
        ));
    };
    let Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(conclusion_left, left_addend),
        IntegerMathTerm::Add(conclusion_right, right_addend),
    ) = conclusion
    else {
        return Err(ProofError::IntegerOrderConclusionMismatch);
    };
    // Both receiving callers validate the conclusion in its exact context.
    // Compare its borrowed children before examining any rule-only payload:
    // a forged hidden addend must not allocate a cloned expected proposition.
    (conclusion_left.as_ref() == left
        && conclusion_right.as_ref() == right
        && left_addend.as_ref() == addend
        && right_addend.as_ref() == addend)
        .then_some(())
        .ok_or(ProofError::IntegerOrderConclusionMismatch)
}

pub(super) fn check_integer_math_order_translation(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerMathOrderTranslation { relation, addend } => IntegerMathOrderTranslation);
    math_order_translation(&relation.conclusion, addend, &proof.conclusion)?;
    Ok(())
}

/// Antisymmetry retains both independently checked orders and their exact
/// endpoints. This is a finite integer relation, never an arithmetic policy
/// rewrite or a way to obtain equality from only one bound.
pub(crate) fn order_antisymmetry(
    forward: &Proposition,
    backward: &Proposition,
    conclusion: &Proposition,
) -> Result<(), ProofError> {
    let Proposition::LessOrEqual(left, right) = forward else {
        return Err(ProofError::RulePremiseMismatch(
            "integer order antisymmetry",
        ));
    };
    let Some(carrier) = left.scalar_type().as_integer() else {
        return Err(ProofError::RulePremiseMismatch(
            "integer order antisymmetry carrier",
        ));
    };
    if carrier.is_address() || right.scalar_type() != ScalarType::Integer(carrier) {
        return Err(ProofError::RulePremiseMismatch(
            "integer order antisymmetry carrier",
        ));
    }
    let Proposition::LessOrEqual(backward_left, backward_right) = backward else {
        return Err(ProofError::RulePremiseMismatch(
            "integer order antisymmetry",
        ));
    };
    if backward_left != right || backward_right != left {
        return Err(ProofError::IntegerOrderMiddleMismatch);
    }
    match conclusion {
        Proposition::Equal(actual_left, actual_right)
            if actual_left == left && actual_right == right =>
        {
            Ok(())
        }
        _ => Err(ProofError::IntegerOrderConclusionMismatch),
    }
}

pub(super) fn check_integer_order_antisymmetry(
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    dispatched_rule!(proof, acceptance, ProofRule::IntegerOrderAntisymmetry { forward, backward } => IntegerOrderAntisymmetry);
    order_antisymmetry(&forward.conclusion, &backward.conclusion, &proof.conclusion)?;
    Ok(())
}
