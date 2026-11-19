use declaration_contracts::trust_declarations::{
    AcceptedContractDeclaration, AcceptedMachineDeclaration, TrustDeclarations,
};
use provider_correspondence::trust::{RootTrustPolicy, reconstruct_trust_obligations};

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

#[test]
fn authored_provider_grants_do_not_reselect_by_display_name() {
    use declaration_contracts::selection::{AuthoredRootGrant, RootGrantSubject};
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
        declaration_contracts::service_surface::ServiceReceiverApplication {
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
        source_span: source_text::SourceSpan::default(),
    };
    let admit_for = |grants: &[AuthoredRootGrant], policy| {
        reconstruct_trust_obligations(&declarations, grants, &[&plan], &selected, &[], policy)
    };
    let admit = |grants: &[AuthoredRootGrant]| admit_for(grants, RootTrustPolicy::Unpackaged);
    let accepted = admit(&[grant.clone(), grant.clone()]).unwrap();
    assert_eq!(accepted.len(), 1);
    // Package review excludes individual claims. A checked provider grant
    // with the same display name still joins its exact provider application.
    assert_eq!(
        admit_for(std::slice::from_ref(&grant), RootTrustPolicy::PackageReview).unwrap(),
        accepted
    );
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
        &[],
        RootTrustPolicy::Unpackaged,
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
fn application_grants_require_the_root_claim_owner() {
    use declaration_contracts::selection::{AuthoredRootGrant, RootGrantSubject};
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
        source_span: source_text::SourceSpan::default(),
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
            &[],
            policy,
        )
    };
    assert_eq!(
        admit(
            own_grant.clone(),
            RootTrustPolicy::Application(root.digest())
        )
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        admit(imported_grant.clone(), RootTrustPolicy::Unpackaged)
            .unwrap()
            .len(),
        1
    );
    let mut forged_owner = imported_grant.clone();
    forged_owner.subject_package = Some(root);
    let observed = [
        (
            "dependency claim",
            admit(imported_grant, RootTrustPolicy::Application(root.digest())).unwrap_err(),
            "package claims require complete package-level review",
        ),
        (
            "package self grant",
            admit(own_grant, RootTrustPolicy::PackageReview).unwrap_err(),
            "package claims require complete package-level review",
        ),
        (
            "forged owner",
            admit(forged_owner, RootTrustPolicy::Application(root.digest())).unwrap_err(),
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
