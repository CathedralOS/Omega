//! Admission consumes the author’s declaration, handle and package. It
//! rejoins those exact coordinates and validates only requested evidence.

use super::{RootTrustPolicy, TrustAdmission, reconstruct_trust_obligations};
use symbols::root_grants::{AuthoredRootGrant, RootGrantSubject};
use symbols::trust_declarations::AcceptedContractDeclaration;
use symbols::trust_declarations::AcceptedMachineDeclaration;
use symbols::trust_declarations::AcceptedTemplateDeclaration;
use symbols::trust_declarations::TrustDeclarationCardinality;
use symbols::trust_declarations::TrustDeclarations;

fn machine(index: u32, name: &str) -> AcceptedMachineDeclaration {
    AcceptedMachineDeclaration {
        machine: symbols::SymbolHandle::from_arena_index(index),
        declaration_identity: name.to_owned(),
        package_identity: None,
        name: name.to_owned(),
        template: Ok(None),
        contract: Ok(AcceptedContractDeclaration {
            report_fingerprint: 37,
            commitment: [7; 32],
        }),
    }
}

fn claim(index: u32, declaration: &str) -> AuthoredRootGrant {
    AuthoredRootGrant {
        selector: declaration.to_owned(),
        subject: RootGrantSubject::AcceptedMachine {
            declaration: declaration.to_owned(),
            machine: symbols::SymbolHandle::from_arena_index(index),
        },
        subject_package: None,
        selecting_machine: symbols::SymbolHandle::from_arena_index(9),
        source_span: Default::default(),
    }
}

fn admit(
    machines: Vec<AcceptedMachineDeclaration>,
    grants: &[AuthoredRootGrant],
    policy: RootTrustPolicy,
) -> Result<Vec<TrustAdmission>, String> {
    let declarations = TrustDeclarations {
        machines: machines.into_boxed_slice(),
    };
    reconstruct_trust_obligations(
        &declarations,
        grants,
        &[],
        &selected_provider_plans::SelectedProviderPlanFacts::default(),
        &[],
        policy,
    )
    .map_err(|errors| format!("{errors:?}"))
}

#[test]
fn identical_authored_grants_collapse_without_resolving_the_display_name() {
    let declarations = vec![
        machine(1, "Claim"),
        machine(2, "left::Claim"),
        machine(3, "right::Claim"),
    ];
    let mut grant = claim(2, "left::Claim");
    grant.selector = "Claim".to_owned();
    let rows = admit(
        declarations,
        &[grant.clone(), grant],
        RootTrustPolicy::Unpackaged,
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].commitment(), "accepted fact: left::Claim");
}

#[test]
fn unrequested_declaration_failures_remain_deferred() {
    let mut unused = machine(2, "Unused");
    unused.template = Err(TrustDeclarationCardinality::Duplicate);
    unused.contract = Err(TrustDeclarationCardinality::Missing);
    assert_eq!(
        admit(
            vec![machine(1, "Claim"), unused.clone()],
            &[claim(1, "Claim")],
            RootTrustPolicy::Unpackaged
        )
        .unwrap()
        .len(),
        1
    );
    assert!(
        admit(vec![unused], &[], RootTrustPolicy::Unpackaged)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn missing_and_duplicate_template_classifications_refuse_the_named_grant() {
    for (cardinality, reason) in [
        (
            TrustDeclarationCardinality::Missing,
            "no pre-lowering template classification",
        ),
        (
            TrustDeclarationCardinality::Duplicate,
            "duplicate pre-lowering template classifications",
        ),
    ] {
        let mut declaration = machine(1, "Claim");
        declaration.template = Err(cardinality);
        let error = admit(
            vec![declaration],
            &[claim(1, "Claim")],
            RootTrustPolicy::Unpackaged,
        )
        .unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
}

#[test]
fn missing_and_duplicate_contract_plans_refuse_only_concrete_claims() {
    for (cardinality, reason) in [
        (
            TrustDeclarationCardinality::Missing,
            "no exact checked machine contract plan",
        ),
        (
            TrustDeclarationCardinality::Duplicate,
            "duplicate exact checked machine contract plans",
        ),
    ] {
        let mut declaration = machine(1, "Claim");
        declaration.contract = Err(cardinality);
        let error = admit(
            vec![declaration.clone()],
            &[claim(1, "Claim")],
            RootTrustPolicy::Unpackaged,
        )
        .unwrap_err();
        assert!(error.contains(reason), "{error}");
        declaration.template = Ok(Some(AcceptedTemplateDeclaration {
            report_fingerprint: 71,
            commitment: [11; 32],
        }));
        assert_eq!(
            admit(
                vec![declaration],
                &[claim(1, "Claim")],
                RootTrustPolicy::Unpackaged
            )
            .unwrap()
            .len(),
            1
        );
    }
}

#[test]
fn package_policy_precedes_deferred_template_evidence() {
    let mut declaration = machine(1, "Claim");
    declaration.template = Err(TrustDeclarationCardinality::Missing);
    let error = admit(
        vec![declaration],
        &[claim(1, "Claim")],
        RootTrustPolicy::PackageReview,
    )
    .unwrap_err();
    assert!(
        error.contains("package-aware compilation cannot admit individual accepted machine"),
        "{error}"
    );
    assert!(!error.contains("pre-lowering template classification"));
}

#[test]
fn exact_declaration_identity_and_invalid_symbols_still_refuse() {
    let error = admit(
        vec![machine(1, "First"), machine(1, "Second")],
        &[claim(1, "First")],
        RootTrustPolicy::Unpackaged,
    )
    .unwrap_err();
    assert!(error.contains("2 exact typed definitions"), "{error}");
    let error = admit(
        vec![machine(0, "Claim")],
        &[claim(0, "Claim")],
        RootTrustPolicy::Unpackaged,
    )
    .unwrap_err();
    assert!(error.contains("no valid exact symbol"), "{error}");
    let error = admit(
        vec![machine(1, "Claim")],
        &[claim(2, "Other")],
        RootTrustPolicy::Unpackaged,
    )
    .unwrap_err();
    assert!(error.contains("has no exact typed definition"), "{error}");
}

#[test]
fn role_and_strong_commitment_define_trust_identity_not_report_fingerprints() {
    let concrete = admit(
        vec![machine(1, "Claim")],
        &[claim(1, "Claim")],
        RootTrustPolicy::Unpackaged,
    )
    .unwrap()
    .remove(0);
    let mut generic = machine(1, "Claim");
    generic.template = Ok(Some(AcceptedTemplateDeclaration {
        report_fingerprint: 37,
        commitment: [7; 32],
    }));
    let template = admit(
        vec![generic],
        &[claim(1, "Claim")],
        RootTrustPolicy::Unpackaged,
    )
    .unwrap()
    .remove(0);
    assert_ne!(concrete.digest(), template.digest());
    let mut different_report = machine(1, "Claim");
    different_report
        .contract
        .as_mut()
        .unwrap()
        .report_fingerprint = 99;
    assert_eq!(
        concrete,
        admit(
            vec![different_report],
            &[claim(1, "Claim")],
            RootTrustPolicy::Unpackaged
        )
        .unwrap()
        .remove(0)
    );
    let mut different_contract = machine(1, "Claim");
    different_contract.contract.as_mut().unwrap().commitment = [8; 32];
    assert_ne!(
        concrete,
        admit(
            vec![different_contract],
            &[claim(1, "Claim")],
            RootTrustPolicy::Unpackaged
        )
        .unwrap()
        .remove(0)
    );
}

#[test]
fn an_absent_authored_handle_precedes_final_contract_validation() {
    let mut declaration = machine(1, "Claim");
    declaration.contract = Err(TrustDeclarationCardinality::Missing);
    let error = admit(
        vec![declaration],
        &[claim(1, "Claim"), claim(2, "Unknown")],
        RootTrustPolicy::Unpackaged,
    )
    .unwrap_err();
    assert!(error.contains("has no exact typed definition"), "{error}");
    assert!(!error.contains("checked machine contract plan"));
}

#[test]
fn an_accepted_machine_handle_cannot_replace_the_retained_declaration() {
    let declarations = TrustDeclarations {
        machines: vec![
            machine(1, "package-a::Claim"),
            machine(2, "package-b::Claim"),
        ]
        .into_boxed_slice(),
    };
    let grant = AuthoredRootGrant {
        selector: "Claim".into(),
        subject: RootGrantSubject::AcceptedMachine {
            declaration: "package-a::Claim".into(),
            machine: symbols::SymbolHandle::from_arena_index(2),
        },
        subject_package: None,
        selecting_machine: symbols::SymbolHandle::from_arena_index(9),
        source_span: Default::default(),
    };
    let errors = reconstruct_trust_obligations(
        &declarations,
        &[grant],
        &[],
        &selected_provider_plans::SelectedProviderPlanFacts::default(),
        &[],
        super::RootTrustPolicy::Unpackaged,
    )
    .unwrap_err();
    assert!(
        errors[0]
            .message
            .contains("changes its exact declaration identity"),
        "{errors:?}"
    );
}

#[test]
fn native_admission_keeps_the_checked_promise_and_selected_binding() {
    use semantic_vocabulary::provider_identity::ProviderPlanDigest;
    let admit = |contract, plan| {
        TrustAdmission::for_native_requirement(
            "native requirement: Root::allocate".into(),
            37,
            &contract,
            ProviderPlanDigest::from_digest(plan),
        )
    };
    let actual = admit([1; 32], [2; 32]).unwrap();
    assert_ne!(
        actual,
        admit([3; 32], [2; 32]).unwrap(),
        "changing a scalar promise invalidates admission even when ABI and locator are unchanged"
    );
    assert_ne!(
        actual,
        admit([1; 32], [4; 32]).unwrap(),
        "changing selected execution invalidates admission under the same promise"
    );
    assert!(
        admit([0; 32], [2; 32])
            .unwrap_err()
            .contains("complete checked requirement contract")
    );
    let provider = TrustAdmission::for_provider_plan(
        "native requirement: Root::allocate".into(),
        37,
        ProviderPlanDigest::from_digest([2; 32]),
    )
    .unwrap();
    assert_ne!(
        actual, provider,
        "provider and native requirement grants keep distinct trust domains"
    );
}
