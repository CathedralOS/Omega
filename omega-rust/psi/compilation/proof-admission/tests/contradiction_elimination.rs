//! Scoped contradiction elimination
//! (`wiki/spec/proofs/classicality.md#contradiction-elimination`): an arrival
//! whose premises contradict each other closes any well-formed goal, but only
//! through a checked derivation from premises that are actually in scope.
//!
//! The derivation is the one the specification names: transitivity of
//! `3 <= counter` and `counter < 3` gives the closed relation `3 < 3`, whose
//! predicate denotation is `Falsehood`; that converts to `Falsehood AND goal`,
//! and conjunction elimination yields the goal. Each negative removes one
//! condition the ratified text imposes.

use proof_admission::{ProofError, ProofNode, ProofRule, accept_certificate, check_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

const COUNTER: u64 = 1;
const OTHER: u64 = 2;

fn carrier() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(carrier()),
    )
}

fn literal(number: u64) -> ScalarTerm {
    ScalarTerm::integer(carrier(), IntegerValue::Unsigned(number.into())).unwrap()
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types([COUNTER, OTHER].map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(carrier()),
        )
    }))
    .unwrap()
}

/// The entry fact `3 <= counter`.
fn lower_bound() -> Proposition {
    Proposition::LessOrEqual(literal(3), value(COUNTER))
}

/// A branch fact `counter < bound`; only `bound <= 3` contradicts the entry fact.
fn upper_bound(bound: u64) -> Proposition {
    Proposition::LessThan(value(COUNTER), literal(bound))
}

/// An obligation unrelated to `counter`, such as `200 <= other`, that the
/// arrival's premises cannot otherwise prove.
fn goal() -> Proposition {
    Proposition::LessOrEqual(literal(200), value(OTHER))
}

fn assumption(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index },
    }
}

/// `3 <= counter`, `counter < bound` ⊢ `3 < bound` ⊢ `Falsehood AND goal` ⊢ `goal`,
/// citing the lower bound at `lower` and the branch bound at `upper`.
fn eliminate(goal: Proposition, bound: u64, lower: usize, upper: usize) -> ProofNode {
    let closed = ProofNode {
        conclusion: Proposition::LessThan(literal(3), literal(bound)),
        rule: ProofRule::IntegerStrictOrderTransitivity {
            left_to_middle: Box::new(assumption(lower_bound(), lower)),
            middle_to_right: Box::new(assumption(upper_bound(bound), upper)),
        },
    };
    let widened = ProofNode {
        conclusion: Proposition::Conjunction(vec![Proposition::Falsehood, goal.clone()]),
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(closed),
        },
    };
    ProofNode {
        conclusion: goal,
        rule: ProofRule::ConjunctionElimination {
            conjunction: Box::new(widened),
            conjunct: 1,
        },
    }
}

#[test]
fn contradictory_premises_close_an_unrelated_goal_and_retain_both_citations() {
    let premises = [lower_bound(), upper_bound(3)];
    let proof = eliminate(goal(), 3, 0, 1);
    let accepted = accept_certificate(&context(), &goal(), &premises, &[], &proof).unwrap();
    let cited: Vec<usize> = accepted
        .assumptions
        .iter()
        .map(|premise| premise.index)
        .collect();
    assert_eq!(cited, [0, 1], "both bound citations remain dependencies");
}

#[test]
fn consistent_premises_do_not_prove_falsehood() {
    // `3 <= counter < 4` holds for counter = 3: transitivity gives the true
    // relation `3 < 4`, which cannot denote `Falsehood AND goal`.
    let premises = [lower_bound(), upper_bound(4)];
    let proof = eliminate(goal(), 4, 0, 1);
    assert!(check_certificate(&context(), &goal(), &premises, &[], &proof).is_err());
}

#[test]
fn a_missing_premise_is_not_a_contradiction() {
    // Only the entry fact is available; the branch fact was never supplied.
    let premises = [lower_bound()];
    let proof = eliminate(goal(), 3, 0, 1);
    assert!(check_certificate(&context(), &goal(), &premises, &[], &proof).is_err());
}

#[test]
fn a_cited_premise_must_be_the_supplied_one() {
    // Citing the supplied roster under the wrong position claims a premise
    // the arrival does not have.
    let premises = [upper_bound(3), lower_bound()];
    let proof = eliminate(goal(), 3, 0, 1);
    assert!(matches!(
        check_certificate(&context(), &goal(), &premises, &[], &proof),
        Err(ProofError::AssumptionConclusionMismatch(_))
    ));
}

#[test]
fn a_branch_assumption_closes_only_its_own_implication() {
    // Inside `counter < 3 -> goal`, the branch fact is a discharged local
    // assumption: the implication is proved and cites only the entry fact.
    let scoped = Proposition::Implication {
        premise: Box::new(upper_bound(3)),
        conclusion: Box::new(goal()),
    };
    let proof = ProofNode {
        conclusion: scoped.clone(),
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(eliminate(goal(), 3, 0, 1)),
        },
    };
    let premises = [lower_bound()];
    let accepted = accept_certificate(&context(), &scoped, &premises, &[], &proof).unwrap();
    let cited: Vec<usize> = accepted
        .assumptions
        .iter()
        .map(|premise| premise.index)
        .collect();
    assert_eq!(
        cited,
        [0],
        "the discharged branch fact is not an ambient premise"
    );

    // The same body cannot justify the bare goal outside that scope.
    let escaped = eliminate(goal(), 3, 0, 1);
    assert!(check_certificate(&context(), &goal(), &premises, &[], &escaped).is_err());
}

#[test]
fn contradiction_does_not_waive_goal_well_formedness() {
    // The goal names a value the arrival's context does not declare.
    let malformed = Proposition::LessOrEqual(literal(200), value(9));
    let premises = [lower_bound(), upper_bound(3)];
    let proof = eliminate(malformed.clone(), 3, 0, 1);
    assert!(check_certificate(&context(), &malformed, &premises, &[], &proof).is_err());
}
