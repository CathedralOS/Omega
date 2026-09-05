use proof_admission::{ProofRule, accept_certificate};
use semantic_vocabulary::{IntegerSign, IntegerValue, Proposition};

use super::super::model::SearchBudget;
use super::fixture::{fork_join, literal, outer_fork_join, value};

#[test]
fn replay_rejects_definition_and_landing_corruption() {
    let fixture = fork_join(IntegerSign::Unsigned, 16, false, false);
    let outcome = fixture.prove(SearchBudget::default());
    fixture.admit(&outcome);
    let mut proof = outcome.proof.expect("proof");
    let mapped = match &mut proof.rule {
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            ..
        } => left_less_or_equal_middle,
        _ => panic!("exact endpoint is relaxed only at the carrier boundary"),
    };
    let ProofRule::IntegerAffineBound { root_bound, .. } = &mut mapped.rule else {
        panic!("top exact add uses the checked direct affine rule")
    };
    let ProofRule::ConjunctionIntroduction(parts) = &mut root_bound.rule else {
        panic!("top exact add retains ordered endpoint custody")
    };
    let ProofRule::IntegerAffineBound { witness, .. } = &mut parts[0].rule else {
        panic!("left endpoint retains its source-definition witness")
    };
    witness.definition_axioms[0] = usize::MAX;
    assert!(
        accept_certificate(
            &fixture.context,
            &fixture.goal,
            &[],
            &fixture.axioms,
            &proof
        )
        .is_err()
    );

    let valid = fixture.prove(SearchBudget::default()).proof.expect("proof");
    let mut drifted = fixture.axioms.clone();
    drifted[0] = Proposition::Equal(
        value(1, fixture.integer_type),
        literal(fixture.integer_type, IntegerValue::Unsigned(4)),
    );
    assert!(accept_certificate(&fixture.context, &fixture.goal, &[], &drifted, &valid).is_err());
}

#[test]
fn replay_rejects_computed_join_identity_order_and_definition_drift() {
    let fixture = outer_fork_join(IntegerSign::Unsigned, 16, false, false);
    let outcome = fixture.prove(SearchBudget::default());
    fixture.admit(&outcome);

    let mut unknown = outcome.proof.clone().expect("proof");
    *computed_join_rule(&mut unknown).2 = usize::MAX;
    assert!(
        accept_certificate(
            &fixture.context,
            &fixture.goal,
            &[],
            &fixture.axioms,
            &unknown,
        )
        .is_err()
    );

    let mut reordered = outcome.proof.clone().expect("proof");
    let (left, right, _) = computed_join_rule(&mut reordered);
    std::mem::swap(left, right);
    assert!(
        accept_certificate(
            &fixture.context,
            &fixture.goal,
            &[],
            &fixture.axioms,
            &reordered,
        )
        .is_err()
    );

    let mut drifted = fixture.axioms.clone();
    drifted[6] = Proposition::Equal(
        value(7, fixture.integer_type),
        semantic_vocabulary::ScalarTerm::exact_integer_add(
            fixture.integer_type,
            value(5, fixture.integer_type),
            value(5, fixture.integer_type),
        )
        .expect("drifted computed join"),
    );
    assert!(
        accept_certificate(
            &fixture.context,
            &fixture.goal,
            &[],
            &drifted,
            outcome.proof.as_ref().expect("proof"),
        )
        .is_err()
    );
}

fn computed_join_rule(
    proof: &mut proof_admission::ProofNode,
) -> (
    &mut Box<proof_admission::ProofNode>,
    &mut Box<proof_admission::ProofNode>,
    &mut usize,
) {
    let mapped = match &mut proof.rule {
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            ..
        } => left_less_or_equal_middle,
        _ => panic!("outer exact endpoint is relaxed at the carrier boundary"),
    };
    let ProofRule::IntegerAffineBound { root_bound, .. } = &mut mapped.rule else {
        panic!("outer exact add uses checked direct mapping")
    };
    let ProofRule::ConjunctionIntroduction(parts) = &mut root_bound.rule else {
        panic!("outer exact add retains ordered endpoint proofs")
    };
    let ProofRule::IntegerExactAddDefinitionBound {
        left_bound,
        right_bound,
        definition_axiom,
    } = &mut parts[1].rule
    else {
        panic!("outer right endpoint retains the computed join rule")
    };
    (left_bound, right_bound, definition_axiom)
}
