//! Ordinary and explicit optimizer lowering share one verified entry identity.

use crate::tests::fixtures::hosted::hosted_custody;

#[test]
fn ordinary_and_explicit_optimizer_lowering_share_the_verified_entry() {
    let (artifact, ..) = hosted_custody();
    let ordinary = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: artifact.semantic_bytes(),
            proof_bytes: artifact.proof_bytes(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_plan())
    .expect("ordinary native lowering produces a bare abstract plan");
    let explicit = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: artifact.semantic_bytes(),
            proof_bytes: artifact.proof_bytes(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .expect("an explicit optimizer request retains verified context");

    assert_eq!(ordinary.entry, explicit.plan().entry);
    assert_eq!(explicit.context().module().entry, explicit.plan().entry);
}
