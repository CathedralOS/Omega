use psi_core::{IntegerSign, IntegerValue, Proposition};
use psi_proof_admission::{ProofRule, accept_certificate};

use super::super::model::SearchBudget;
use super::fixture::{fork_join, literal, value};

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
