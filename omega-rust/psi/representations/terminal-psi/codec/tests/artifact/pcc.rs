use super::{canonical_artifact, kernel_bundle, semantic_module};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ServiceId;
use terminal_codec::{
    PccDependency, PccProofSidecar, PccReceiverPolicy, PccVerificationOutcome,
    PreparedPsiProofPair, PsiProofPairPreparationError, build_psi_proof_sidecar,
    verify_psi_proof_sidecar,
};
use terminal_psi::{InstallationReachDependency, ServiceDeclaration};

#[test]
fn prepared_psi_pair_preserves_exact_bytes_and_receiver_replay() {
    let artifact = canonical_artifact(&semantic_module(), &kernel_bundle(), None);
    let profile = AdmissionProfile::default();
    let expected_artifact = artifact.to_bytes();
    let expected_sidecar = build_psi_proof_sidecar(&artifact, &profile, &expected_artifact)
        .expect("canonical sidecar");
    let pair = PreparedPsiProofPair::new(&artifact, &profile).expect("prepared pair");
    assert_eq!(pair.artifact_bytes(), expected_artifact);
    assert_eq!(pair.sidecar_bytes(), expected_sidecar.to_bytes());
    let policy = PccReceiverPolicy::for_offered_claim(&expected_sidecar, profile);
    assert!(matches!(
        verify_psi_proof_sidecar(pair.artifact_bytes(), pair.sidecar_bytes(), &policy),
        PccVerificationOutcome::Complete(_)
    ));

    let mut substituted_artifact = pair.artifact_bytes().to_vec();
    substituted_artifact[0] ^= 1;
    assert!(matches!(
        verify_psi_proof_sidecar(&substituted_artifact, pair.sidecar_bytes(), &policy),
        PccVerificationOutcome::Reject(_)
    ));
}

#[test]
fn prepared_psi_pair_does_not_override_receiver_policy() {
    let artifact = canonical_artifact(&semantic_module(), &kernel_bundle(), None);
    let profile = AdmissionProfile::default();
    let pair = PreparedPsiProofPair::new(&artifact, &profile).expect("prepared pair");
    let sidecar = PccProofSidecar::from_bytes(pair.sidecar_bytes()).expect("sidecar");
    let mut policy = PccReceiverPolicy::for_offered_claim(&sidecar, profile);
    policy
        .required_guarantees
        .push("receiver::extra_guarantee".into());
    assert!(matches!(
        verify_psi_proof_sidecar(pair.artifact_bytes(), pair.sidecar_bytes(), &policy),
        PccVerificationOutcome::Reject(_)
    ));
}

#[test]
fn prepared_psi_pair_refuses_canonical_but_unproved_artifact() {
    let mut proof = kernel_bundle();
    proof.evidence.clear();
    let artifact = canonical_artifact(&semantic_module(), &proof, None);
    let error = PreparedPsiProofPair::new(&artifact, &AdmissionProfile::default())
        .expect_err("canonical framing cannot substitute for proof");
    assert!(matches!(
        error,
        PsiProofPairPreparationError::ProducerValidation(PccVerificationOutcome::Reject(_))
    ));
    assert!(
        error
            .to_string()
            .starts_with("psi proof sidecar failed producer validation:")
    );
}

#[test]
fn installation_dependency_commitments_follow_service_meaning_not_local_numbers() {
    let service = ServiceId::new(1).unwrap();
    let mut dependency = InstallationReachDependency {
        requirement_identity: "test::observe".into(),
        upper_bound: vec![service],
    };
    let mut services = vec![ServiceDeclaration {
        id: service,
        identity: "Console".into(),
        parents: Vec::new(),
    }];
    let original = PccDependency::from_installation_reach(&dependency, &services).unwrap();
    services[0].identity = "Filesystem".into();
    assert_ne!(
        original,
        PccDependency::from_installation_reach(&dependency, &services).unwrap()
    );
    services[0].identity = "Console".into();
    services[0].id = ServiceId::new(20).unwrap();
    dependency.upper_bound[0] = services[0].id;
    assert_eq!(
        original,
        PccDependency::from_installation_reach(&dependency, &services).unwrap()
    );
    assert!(PccDependency::from_installation_reach(&dependency, &[]).is_err());
}
