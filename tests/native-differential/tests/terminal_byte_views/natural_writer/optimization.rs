//! Authored grouped ranking remains authoritative through current-IR replay.
use super::*;

fn verified_writer() -> terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
    let lowered = writer();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

#[test]
fn natural_writer_optimizer_replays_verified_component_without_countdown_certificate() {
    let verified = verified_writer();
    let custody = abstract_operations_to_abstract_operations::validation::validate_verified_psi_cycle_components(&verified)
        .expect("verified natural writer must retain its actual cycle in optimizer admission");
    assert_eq!(custody.components().len(), 1);
    assert!(
        custody.ranking_certificates().certificates().is_empty(),
        "natural proof must not become a countdown certificate"
    );
    assert_eq!(
        abstract_operations_to_abstract_operations::validation::validate_transformed_psi_cycle_components(verified.input(), verified.unit()).unwrap(),
        custody,
    );
}
