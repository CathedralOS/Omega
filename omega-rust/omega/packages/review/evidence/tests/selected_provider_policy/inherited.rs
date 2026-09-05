use super::*;

#[test]
fn inherited_service_keeps_selecting_schema_and_declaring_requirement_owners() {
    let build = r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_provider<SelectedHost, HostProvider>();
}
"#;
    let fixture = Fixture::local(fixtures::INHERITED, build, TargetProfile::WindowsX64);
    let review = project_checked_package_review(&fixture.checked)
        .expect("ordinary package review must preserve the inherited selected schema");
    let reviewed_provider = review
        .selected_providers()
        .iter()
        .find(|provider| provider.schema_declaration().path() == "SelectedHost")
        .expect("ordinary review retains the explicitly selected descendant schema");
    assert_eq!(reviewed_provider.row_declarations().len(), 1);
    assert!(
        reviewed_provider.row_declarations()[0]
            .requirement()
            .path()
            .contains("BaseHost::ping")
    );
    let policy = project(&fixture);
    let plan = policy
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "SelectedHost")
        .unwrap_or_else(|| panic!(
            "explicit SelectedHost selection must survive capture; checked schemas: {:?}; policy schemas: {:?}; provenance: {:?}",
            fixture.checked.selected_provider_plans().plans().iter().map(|plan| (&plan.name, &plan.schema.trait_name)).collect::<Vec<_>>(),
            policy.plans().iter().map(|plan| (plan.plan_name(), plan.schema_declaration().path())).collect::<Vec<_>>(),
            fixture.checked.selected_provider_provenance(),
        ));
    let [method] = plan.methods() else {
        panic!("one inherited service method")
    };
    assert_eq!(method.requirement_owner().path(), "BaseHost");
    assert!(method.requirement().path().contains("BaseHost::ping"));
    let calling = method.calling().expect("complete inherited calling policy");
    assert_eq!(calling.boundary_trait(), plan.schema_declaration());
    assert_eq!(calling.requirement_trait(), method.requirement_owner());
    assert_eq!(calling.requirement(), method.requirement());
    assert_eq!(calling.semantic_parameters().len(), 1);
    assert!(calling.semantic_result().is_some());
    assert_eq!(plan.rows()[0].requirement(), method.requirement());
    assert_eq!(plan.rows()[0].realization().path(), "HostProvider::ping");
    assert_eq!(method.parameter_count(), 1);
    assert_eq!(method.parameter_type_identities().len(), 1);
    assert!(method.has_result());
    assert!(method.result_type_identity().is_some());
    assert!(!method.may_block());
    assert!(!method.may_suspend());
    assert!(method.entry_claims().is_empty());
    assert!(method.result_claims().is_empty());
    assert!(method.synchronous_invocations().is_empty());
    assert!(method.termination_premises().is_empty());
    let checked_plan = fixture
        .checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|checked| checked.name == plan.plan_name())
        .unwrap();
    let checked_method = &checked_plan.schema.methods[0];
    assert_eq!(
        method.parameter_type_identities(),
        checked_method.parameter_type_identities
    );
    assert_eq!(
        method.result_type_identity(),
        checked_method.result_type_identity.as_deref()
    );
    assert_eq!(method.entry_claims(), checked_method.entry_claims);
    assert_eq!(method.result_claims(), checked_method.result_claims);
    assert_eq!(method.service_reach(), checked_method.service_reach);
    assert_eq!(
        method.synchronous_invocations(),
        checked_method.synchronous_invocations
    );
    assert_eq!(
        method.terminates_guarantee(),
        checked_method.terminates_guarantee
    );
    assert_eq!(
        method.termination_premises(),
        checked_method.termination_premises
    );
    let realization = fixture
        .checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| {
            fixture.checked.symbols.name(realization.boundary_trait) == "SelectedHost"
        })
        .unwrap();
    assert_eq!(
        calling,
        &omega_package_evidence::project_checked_calling_policy(&fixture.checked, realization)
            .unwrap()
    );

    let changed_source = fixtures::INHERITED.replace(
        "machine ping(value: u64) -> u64;",
        "machine ping(value: u64) -> u64 blocks;",
    );
    let changed = project(&Fixture::local(
        &changed_source,
        build,
        TargetProfile::WindowsX64,
    ));
    let changed_plan = changed
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "SelectedHost")
        .unwrap_or_else(|| {
            panic!("changed inherited service lost explicitly selected schema: {changed:#?}")
        });
    assert!(changed_plan.methods()[0].may_block());
    assert_eq!(
        calling,
        changed_plan.methods()[0].calling().unwrap(),
        "service blocking ceiling is independent of calling placements"
    );
    assert_ne!(policy, changed);
    assert_ne!(
        policy.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}
