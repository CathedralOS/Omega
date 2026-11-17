// We test `reconstruct_trust_obligations` and `TrustAdmission` across exact vs
// leaf-name grant resolution, deferred unused declaration errors, template vs
// contract cardinality rejections, package-aware non-provider grant rejection,
// and domain-separated digest equality.

use crate::{TrustAdmission, reconstruct_trust_obligations};
use language_semantics::trust_declarations::{
    AcceptedContractDeclaration, AcceptedMachineDeclaration, AcceptedTemplateDeclaration,
    TrustDeclarationCardinality, TrustDeclarations,
};

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

fn admit(
    machines: Vec<AcceptedMachineDeclaration>,
    grants: &[&str],
    package_aware: bool,
) -> Result<Vec<TrustAdmission>, String> {
    let declarations = TrustDeclarations {
        machines: machines.into_boxed_slice(),
    };
    let grants = grants
        .iter()
        .map(|grant| {
            let crate::NonProviderTrustGrant::AcceptedMachine(symbol) =
                crate::resolve_non_provider_trust_grant(&declarations, grant)
                    .map_err(|error| error.message)?;
            let machine = crate::accepted_machine_grants::accepted_machine(&declarations, symbol)
                .map_err(|error| error.message)?;
            Ok(language_semantics::declaration_selection::AuthoredRootGrant {
            selector: (*grant).to_owned(),
            subject: language_semantics::declaration_selection::RootGrantSubject::AcceptedMachine {
                declaration: machine.declaration_identity.clone(), machine: symbol,
            },
            subject_package: machine.package_identity,
            selecting_machine: symbols::SymbolHandle::from_arena_index(0),
            source_span: source::SourceSpan::default(),
        })
        })
        .collect::<Result<Vec<_>, String>>()?;
    reconstruct_trust_obligations(
        &declarations,
        &grants,
        &[],
        &effects::SelectedProviderPlanFacts::default(),
        crate::NativeTrustInputs {
            module: &terminal_psi::TerminalModule::declarations(),
            target: None,
            leaves: &[],
            evaluated: &[],
            identities: &terminal_psi::TerminalIdentityTable::default(),
        },
        if package_aware {
            crate::RootTrustPolicy::PackageReview
        } else {
            crate::RootTrustPolicy::Unpackaged
        },
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

#[test]
fn authored_provider_grants_do_not_reselect_by_display_name() {
    use language_semantics::declaration_selection::{AuthoredRootGrant, RootGrantSubject};
    let mut plan = effects::provider_plan::ProviderPlan {
        name: "linux_x86_64::Selected::satisfies::Service".into(),
        provider_type: "package-a::Selected".into(),
        target: "linux_x86_64".into(),
        ..Default::default()
    };
    plan.provider_application = plan
        .identities
        .intern("package-a::Selected<package-a::Tag>");
    let schema = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    schema.trait_name = schema.identities.intern("Service");
    let argument = schema.identities.intern("package-a::Tag");
    schema.receiver_applications.push(
        language_semantics::service_surface::ServiceReceiverApplication {
            application: schema.identities.intern("Service<package-a::Tag>"),
            trait_name: schema.trait_name,
            trait_package_identity: None,
            argument_identities: vec![argument],
            lifetime_argument_identities: Vec::new(),
            requirement_identities: Vec::new(),
        },
    );
    let selected =
        effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan.clone()]).unwrap();
    let declarations = TrustDeclarations {
        machines: vec![machine(1, "Selected")].into_boxed_slice(),
    };
    let grant = AuthoredRootGrant {
        selector: "Selected".into(),
        subject: RootGrantSubject::Provider {
            declaration: "package-a::Selected".into(),
            application: Some("package-a::Selected<package-a::Tag>".into()),
        },
        subject_package: None,
        selecting_machine: symbols::SymbolHandle::from_arena_index(9),
        source_span: source::SourceSpan::default(),
    };
    let admit = |grants: &[AuthoredRootGrant]| {
        reconstruct_trust_obligations(
            &declarations,
            grants,
            &[&plan],
            &selected,
            crate::NativeTrustInputs {
                module: &terminal_psi::TerminalModule::declarations(),
                target: Some("linux_x86_64"),
                leaves: &[],
                evaluated: &[],
                identities: &terminal_psi::TerminalIdentityTable::default(),
            },
            crate::RootTrustPolicy::Unpackaged,
        )
    };
    assert_eq!(admit(&[grant.clone(), grant.clone()]).unwrap().len(), 1);
    let mut other_owner = grant.clone();
    let RootGrantSubject::Provider { declaration, .. } = &mut other_owner.subject else {
        unreachable!()
    };
    *declaration = "package-b::Selected".into();
    let mut forged_package = grant.clone();
    forged_package.subject_package =
        Some(semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32]).unwrap());
    let mut other_application = grant.clone();
    let RootGrantSubject::Provider { application, .. } = &mut other_application.subject else {
        unreachable!()
    };
    *application = Some("package-a::Selected<package-a::Other>".into());
    let mut other_plan = plan.clone();
    other_plan.provider_application = other_plan
        .identities
        .intern("package-a::Selected<package-a::Other>");
    other_plan.digest_memo = Default::default();
    let schema = std::sync::Arc::make_mut(&mut other_plan.schema.declaration);
    let application = schema.identities.intern("Service<package-a::Other>");
    let argument = schema.identities.intern("package-a::Other");
    schema.receiver_applications[0].application = application;
    schema.receiver_applications[0].argument_identities = vec![argument];
    let both = effects::SelectedProviderPlanFacts::from_selected_plans(vec![
        plan.clone(),
        other_plan.clone(),
    ])
    .unwrap();
    let accepted = reconstruct_trust_obligations(
        &declarations,
        &[grant.clone(), other_application.clone()],
        &[&plan, &other_plan],
        &both,
        crate::NativeTrustInputs {
            module: &terminal_psi::TerminalModule::declarations(),
            target: Some("linux_x86_64"),
            leaves: &[],
            evaluated: &[],
            identities: &terminal_psi::TerminalIdentityTable::default(),
        },
        crate::RootTrustPolicy::Unpackaged,
    )
    .unwrap();
    assert_eq!(
        accepted.len(),
        2,
        "two complete applications coexist despite the same plan display name"
    );
    assert_ne!(accepted[0].commitment(), accepted[1].commitment());
    let mut native = grant;
    native.subject = RootGrantSubject::BoundaryRequirement {
        declaration: "package-a::Selected".into(),
        callable: "Selected()".into(),
    };
    let observed = [
        (
            "owner",
            admit(&[other_owner]).unwrap_err(),
            "0 selected plans",
        ),
        (
            "package metadata",
            admit(&[forged_package]).unwrap_err(),
            "changes its provider package identity",
        ),
        (
            "application",
            admit(&[other_application]).unwrap_err(),
            "0 selected plans",
        ),
        (
            "subject kind",
            admit(&[native]).unwrap_err(),
            "no unique selected native requirement",
        ),
    ];
    for (name, errors, _) in &observed {
        eprintln!("{name}: {errors:?}");
    }
    for (name, errors, expected) in observed {
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{name}: {errors:?}"
        );
    }
}

#[test]
fn an_accepted_machine_handle_cannot_replace_the_retained_declaration() {
    use language_semantics::declaration_selection::{AuthoredRootGrant, RootGrantSubject};
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
        source_span: source::SourceSpan::default(),
    };
    let errors = reconstruct_trust_obligations(
        &declarations,
        &[grant],
        &[],
        &effects::SelectedProviderPlanFacts::default(),
        crate::NativeTrustInputs {
            module: &terminal_psi::TerminalModule::declarations(),
            target: None,
            leaves: &[],
            evaluated: &[],
            identities: &terminal_psi::TerminalIdentityTable::default(),
        },
        crate::RootTrustPolicy::Unpackaged,
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
fn application_grants_require_the_root_claim_owner() {
    use language_semantics::declaration_selection::{AuthoredRootGrant, RootGrantSubject};
    let root = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap();
    let dependency = semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap();
    let mut own = machine(1, "root::Claim");
    own.package_identity = Some(root);
    let mut imported = machine(2, "dependency::Claim");
    imported.package_identity = Some(dependency);
    let grant = |row: &AcceptedMachineDeclaration| AuthoredRootGrant {
        selector: row.name.clone(),
        subject: RootGrantSubject::AcceptedMachine {
            declaration: row.declaration_identity.clone(),
            machine: row.machine,
        },
        subject_package: row.package_identity,
        selecting_machine: symbols::SymbolHandle::from_arena_index(9),
        source_span: source::SourceSpan::default(),
    };
    let own_grant = grant(&own);
    let imported_grant = grant(&imported);
    let declarations = TrustDeclarations {
        machines: vec![own, imported].into_boxed_slice(),
    };
    let admit = |grant, policy| {
        reconstruct_trust_obligations(
            &declarations,
            &[grant],
            &[],
            &effects::SelectedProviderPlanFacts::default(),
            crate::NativeTrustInputs {
                module: &terminal_psi::TerminalModule::declarations(),
                target: None,
                leaves: &[],
                evaluated: &[],
                identities: &terminal_psi::TerminalIdentityTable::default(),
            },
            policy,
        )
    };
    assert_eq!(
        admit(own_grant.clone(), crate::RootTrustPolicy::Application(root))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        admit(imported_grant.clone(), crate::RootTrustPolicy::Unpackaged)
            .unwrap()
            .len(),
        1
    );
    let mut forged_owner = imported_grant.clone();
    forged_owner.subject_package = Some(root);
    let observed = [
        (
            "dependency claim",
            admit(imported_grant, crate::RootTrustPolicy::Application(root)).unwrap_err(),
            "package claims require complete package-level review",
        ),
        (
            "package self grant",
            admit(own_grant, crate::RootTrustPolicy::PackageReview).unwrap_err(),
            "package claims require complete package-level review",
        ),
        (
            "forged owner",
            admit(forged_owner, crate::RootTrustPolicy::Application(root)).unwrap_err(),
            "changes its exact declaration identity",
        ),
    ];
    for (name, errors, _) in &observed {
        eprintln!("{name}: {errors:?}");
    }
    for (name, errors, expected) in observed {
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{name}: {errors:?}"
        );
    }
}

#[test]
fn native_admission_keeps_the_checked_promise_and_selected_binding() {
    use effects::provider_plan::ProviderPlanDigest;
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
