use crate::{TrustAdmission, reconstruct_trust_obligations};
use language_semantics::trust_declarations::{
    AcceptedContractDeclaration, AcceptedMachineDeclaration, AcceptedTemplateDeclaration,
    TrustDeclarationCardinality, TrustDeclarations,
};

fn machine(index: u32, name: &str) -> AcceptedMachineDeclaration {
    AcceptedMachineDeclaration {
        machine: symbols::SymbolHandle::from_arena_index(index),
        name: name.to_owned(),
        template: Ok(None),
        contract: Ok(AcceptedContractDeclaration {
            report_fingerprint: 37,
            commitment: [7; 32],
        }),
    }
}

fn admit(
    machines: Vec<AcceptedMachineDeclaration>,
    grants: &[&str],
    package_aware: bool,
) -> Result<Vec<TrustAdmission>, String> {
    reconstruct_trust_obligations(
        &TrustDeclarations {
            machines: machines.into_boxed_slice(),
        },
        &grants
            .iter()
            .map(|grant| (*grant).to_owned())
            .collect::<Vec<_>>(),
        &[],
        &effects::SelectedProviderPlanFacts::default(),
        package_aware,
    )
    .map_err(|errors| format!("{errors:?}"))
}

#[test]
fn exact_name_precedes_ambiguous_leaf_and_duplicate_grants_collapse() {
    let declarations = vec![
        machine(1, "Claim"),
        machine(2, "left::Claim"),
        machine(3, "right::Claim"),
    ];
    let rows = admit(declarations, &["Claim", "Claim"], false).expect("exact declaration");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].commitment(), "accepted fact: Claim");
    let error = admit(
        vec![machine(2, "left::Claim"), machine(3, "right::Claim")],
        &["Claim"],
        false,
    )
    .unwrap_err();
    assert!(
        error.contains("ambiguous across non-provider trust subjects"),
        "{error}"
    );
}

#[test]
fn unrequested_declaration_failures_remain_deferred() {
    let mut unused = machine(2, "Unused");
    unused.template = Err(TrustDeclarationCardinality::Duplicate);
    unused.contract = Err(TrustDeclarationCardinality::Missing);
    assert_eq!(
        admit(vec![machine(1, "Claim"), unused.clone()], &["Claim"], false)
            .unwrap()
            .len(),
        1
    );
    assert!(admit(vec![unused], &[], false).unwrap().is_empty());
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
        let error = admit(vec![declaration], &["Claim"], false).unwrap_err();
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
        let error = admit(vec![declaration.clone()], &["Claim"], false).unwrap_err();
        assert!(error.contains(reason), "{error}");
        declaration.template = Ok(Some(AcceptedTemplateDeclaration {
            report_fingerprint: 71,
            commitment: [11; 32],
        }));
        assert_eq!(
            admit(vec![declaration], &["Claim"], false).unwrap().len(),
            1
        );
    }
}

#[test]
fn package_policy_precedes_deferred_template_evidence() {
    let mut declaration = machine(1, "Claim");
    declaration.template = Err(TrustDeclarationCardinality::Missing);
    let error = admit(vec![declaration], &["Claim"], true).unwrap_err();
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
        &["First"],
        false,
    )
    .unwrap_err();
    assert!(error.contains("2 exact typed definitions"), "{error}");
    let error = admit(vec![machine(0, "Claim")], &["Claim"], false).unwrap_err();
    assert!(error.contains("no valid exact symbol"), "{error}");
    let error = admit(vec![machine(1, "Claim")], &["Other"], false).unwrap_err();
    assert!(
        error.contains("does not name an exact accepted machine or selected provider plan"),
        "{error}"
    );
}

#[test]
fn role_and_strong_commitment_define_trust_identity_not_report_fingerprints() {
    let concrete = admit(vec![machine(1, "Claim")], &["Claim"], false)
        .unwrap()
        .remove(0);
    let mut generic = machine(1, "Claim");
    generic.template = Ok(Some(AcceptedTemplateDeclaration {
        report_fingerprint: 37,
        commitment: [7; 32],
    }));
    let template = admit(vec![generic], &["Claim"], false).unwrap().remove(0);
    assert_ne!(concrete.digest(), template.digest());
    let mut different_report = machine(1, "Claim");
    different_report
        .contract
        .as_mut()
        .unwrap()
        .report_fingerprint = 99;
    assert_eq!(
        concrete,
        admit(vec![different_report], &["Claim"], false)
            .unwrap()
            .remove(0)
    );
    let mut different_contract = machine(1, "Claim");
    different_contract.contract.as_mut().unwrap().commitment = [8; 32];
    assert_ne!(
        concrete,
        admit(vec![different_contract], &["Claim"], false)
            .unwrap()
            .remove(0)
    );
}

#[test]
fn selector_failures_precede_final_contract_validation() {
    let mut declaration = machine(1, "Claim");
    declaration.contract = Err(TrustDeclarationCardinality::Missing);
    let error = admit(vec![declaration], &["Claim", "Unknown"], false).unwrap_err();
    assert!(error.contains("root grant `Unknown`"), "{error}");
    assert!(!error.contains("checked machine contract plan"));
}
