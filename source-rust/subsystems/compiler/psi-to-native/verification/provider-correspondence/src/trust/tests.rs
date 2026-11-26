use symbols::root_grants::{AuthoredRootGrant, RootGrantSubject};

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

#[test]
fn locked_provider_grants_require_the_selected_application() {
    let selected = selected_provider_plans::SelectedProviderPlanFacts::default();
    let mut grant = claim(2, "DisplayName");
    grant.subject = RootGrantSubject::Provider {
        declaration: "Package::Hosted".into(),
        application: Some("Hosted<8>".into()),
    };
    let error = super::selected_provider_grants(&selected, &[grant]).unwrap_err();
    assert!(
        error
            .message
            .contains("0 selected plans for its exact provider declaration and application")
    );
}

#[test]
fn an_accepted_machine_needs_its_own_grant_review() {
    let selected = selected_provider_plans::SelectedProviderPlanFacts::default();
    let error = super::selected_provider_grants(&selected, &[claim(2, "Claim")]).unwrap_err();
    assert!(error.message.contains("requires its own admission check"));
}

#[test]
fn reviewed_provider_grants_require_the_selected_application_and_physical_owner() {
    use provider_plans::{ProviderPlan, ServiceReceiverApplication};
    use selected_provider_plans::SelectedProviderPlanFacts;
    use semantic_vocabulary::PackageKeyIdentity;
    let owner = PackageKeyIdentity::from_digest([17; 32]).unwrap();
    let mut plan = ProviderPlan {
        name: "MetricReader".into(),
        provider_type: "Reader".into(),
        target: "linux_x86_64".into(),
        provider_type_package_identity: Some(owner),
        ..Default::default()
    };
    plan.provider_application = plan.identities.intern("Reader<17>");
    let service = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    service.trait_name = service.identities.intern("Metric");
    service
        .receiver_applications
        .push(ServiceReceiverApplication {
            application: service.trait_name,
            trait_name: service.trait_name,
            trait_package_identity: None,
            argument_identities: Vec::new(),
            lifetime_argument_identities: Vec::new(),
            requirement_identities: Vec::new(),
        });
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![plan]).unwrap();
    let mut grant = claim(1, "same text for every candidate");
    grant.subject_package = Some(owner);
    grant.subject = RootGrantSubject::Provider {
        declaration: "Reader".into(),
        application: Some("Reader<17>".into()),
    };
    assert_eq!(
        super::selected_provider_grants(&selected, std::slice::from_ref(&grant)).unwrap(),
        [(0, Some("Reader<17>"))],
    );
    let refusal = |grant: &AuthoredRootGrant| {
        super::selected_provider_grants(&selected, std::slice::from_ref(grant))
            .unwrap_err()
            .message
    };
    grant.subject_package = PackageKeyIdentity::from_digest([29; 32]);
    assert!(refusal(&grant).contains("package identity"));
    grant.subject_package = Some(owner);
    grant.subject = RootGrantSubject::Provider {
        declaration: "Reader".into(),
        application: Some("Reader<29>".into()),
    };
    assert!(refusal(&grant).contains("0 selected plans"));
}
