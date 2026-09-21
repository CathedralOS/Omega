//! The bare plan and the optimizer downgrade of one admission share one
//! verified entry identity.

use crate::tests::fixtures::hosted::hosted_custody;

#[test]
fn bare_plan_and_optimizer_downgrade_share_the_verified_entry() {
    let (artifact, ..) = hosted_custody();
    let admit = || {
        terminal_psi_to_abstract_operations::lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: artifact.semantic_bytes(),
                proof_bytes: artifact.proof_bytes(),
                obligation_ledger_bytes: None,
            },
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("native admission of the hosted artifact")
    };
    let plan = admit().into_plan();
    let optimizer = admit()
        .into_optimization_artifact()
        .into_optimization_input();

    assert_eq!(plan.entry, optimizer.plan().entry);
    assert_eq!(optimizer.context().module().entry, optimizer.plan().entry);
}
