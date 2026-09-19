use super::{canonical_artifact, kernel_bundle, operation_id, semantic_module};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{BoundaryMachineId, ServiceId};
use terminal_codec::{
    CanonicalTerminalArtifact, PSI_TERMINAL_VERIFIED_GUARANTEE, PccDependency, PccGuarantee,
    PccProofSidecar, PccReceiverPolicy, PccVerificationOutcome, admission_profile_identity,
    build_psi_proof_sidecar, psi_semantic_profile_identity, terminal_assumption_closure,
    verify_psi_proof_sidecar,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, InstallationReachDependency, Operation,
    OperationKind, OperationResult, ServiceDeclaration,
};

fn artifact_and_receiver() -> (CanonicalTerminalArtifact, PccReceiverPolicy) {
    let mut module = semantic_module();
    let boundary = BoundaryMachineId::new(1).expect("boundary identity");
    let service = ServiceId::new(1).expect("service identity");
    let dependency = InstallationReachDependency {
        requirement_identity: "test::observe".into(),
        upper_bound: vec![service],
    };
    module.services.push(ServiceDeclaration {
        id: service,
        identity: "test::Observation".into(),
        parents: Vec::new(),
    });
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary,
        identity: dependency.requirement_identity.clone(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: vec![service],
    });
    module.machines[0].published_service_ceiling = vec![service];
    module.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(2),
        static_reach_binding: None,
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    module.root_service_reach.installation_dependencies = vec![dependency.clone()];
    let artifact = canonical_artifact(&module, &kernel_bundle(), None);
    let profile = AdmissionProfile::default();
    // Receiver choices come from the test's independently fixed contract, never
    // from the sidecar offered by the adversary.
    let policy = PccReceiverPolicy {
        policy_package_identity: "test::receiver".into(),
        configuration_identity: "terminal-verification".into(),
        required_guarantees: vec![PSI_TERMINAL_VERIFIED_GUARANTEE.into()],
        admitted_premises: Vec::new(),
        accepted_semantic_profiles: vec![psi_semantic_profile_identity(&artifact).unwrap()],
        accepted_checker_profiles: vec![admission_profile_identity(&profile)],
        admitted_assumptions: terminal_assumption_closure(),
        possessed_dependencies: vec![
            PccDependency::from_installation_reach(&dependency, &module.services).unwrap(),
        ],
        admission_profile: profile,
        max_artifact_bytes: u64::MAX,
        max_evidence_bytes: u64::MAX,
    };
    (artifact, policy)
}

#[test]
fn psi_pcc_reconstructs_every_claim_field_including_omissions() {
    let (artifact, policy) = artifact_and_receiver();
    let bytes = artifact.to_bytes();
    let honest = build_psi_proof_sidecar(&artifact, &policy.admission_profile, &bytes).unwrap();
    assert!(!honest.assumptions().is_empty());
    assert!(!honest.dependencies().is_empty());
    assert!(matches!(
        verify_psi_proof_sidecar(&bytes, &honest.to_bytes(), &policy),
        PccVerificationOutcome::Complete(_)
    ));

    for subject in [
        "semantic profile",
        "checker profile",
        "guarantees",
        "premises",
        "assumption closure",
        "dependency inventory",
    ] {
        let mut receiver = policy.clone();
        let mut semantic_profile = honest.semantic_profile().to_owned();
        let mut checker_profile = honest.checker_profile().to_owned();
        let mut guarantees = honest.guarantees().to_vec();
        let mut assumptions = honest.assumptions().to_vec();
        let mut dependencies = honest.dependencies().to_vec();
        match subject {
            "semantic profile" => {
                semantic_profile = "unsupported-semantics".into();
                receiver
                    .accepted_semantic_profiles
                    .push(semantic_profile.clone());
            }
            "checker profile" => {
                checker_profile = "different-checker".into();
                receiver
                    .accepted_checker_profiles
                    .push(checker_profile.clone());
            }
            "guarantees" => {
                guarantees.push(PccGuarantee {
                    identity: "test::unproved-guarantee".into(),
                    premises: Vec::new(),
                });
                receiver
                    .required_guarantees
                    .push("test::unproved-guarantee".into());
            }
            "premises" => {
                guarantees[0].premises.push("test::invented-premise".into());
                receiver
                    .admitted_premises
                    .push("test::invented-premise".into());
            }
            "assumption closure" => {
                assumptions.clear();
                receiver.admitted_assumptions.clear();
            }
            "dependency inventory" => {
                dependencies.clear();
                receiver.possessed_dependencies.clear();
            }
            _ => unreachable!("test mutation"),
        }
        let forged = PccProofSidecar::new(
            honest.product(),
            *honest.artifact_commitment(),
            semantic_profile,
            checker_profile,
            guarantees,
            Vec::new(),
            assumptions,
            dependencies,
        )
        .unwrap();
        let outcome = verify_psi_proof_sidecar(&bytes, &forged.to_bytes(), &receiver);
        let PccVerificationOutcome::Reject(rejection) = outcome else {
            panic!("forged {subject} accepted: {outcome:?}");
        };
        assert_eq!(
            rejection.subject,
            if subject == "premises" {
                "guarantees"
            } else {
                subject
            }
        );
    }
}

#[test]
fn psi_pcc_does_not_infer_receiver_authorization_from_valid_evidence() {
    let (artifact, mut receiver) = artifact_and_receiver();
    let bytes = artifact.to_bytes();
    let honest = build_psi_proof_sidecar(&artifact, &receiver.admission_profile, &bytes).unwrap();
    receiver.possessed_dependencies.clear();
    assert!(matches!(
        verify_psi_proof_sidecar(&bytes, &honest.to_bytes(), &receiver),
        PccVerificationOutcome::Reject(rejection) if rejection.subject == "dependency"
    ));
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
