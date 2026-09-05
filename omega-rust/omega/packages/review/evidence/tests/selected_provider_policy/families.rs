use super::*;

#[test]
fn unused_selected_plan_retains_its_explicit_grant() {
    let source = r#"boundary trait Host { machine ping(); }
machine ping_leaf() satisfies Host::ping via Binding::DllImport("omega-test", "host_ping");
"#;
    let granted_build = r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.accept_boundary<windows_x86_64::satisfies::Host>();
}
"#;
    let granted = Fixture::local(source, granted_build, TargetProfile::WindowsX64);
    assert_eq!(granted.checked.selected_provider_grants().len(), 1);
    let policy = project(&granted);
    let [plan] = policy.plans() else {
        panic!("one unused selected plan")
    };
    assert_eq!(
        plan.grants(),
        &[PackageReviewProviderGrantSelectorKind::PlanName]
    );
    assert_eq!(plan.rows().len(), 1);
    assert!(policy.families().is_empty());
    assert!(
        granted
            .checked
            .boundary_calling_plan_realizations()
            .is_empty()
    );
    assert!(
        project_checked_selected_provider_policy(
            &granted.changed_build_target("accept_boundary#", "ignored_grant"),
            granted.target,
            package_identity(),
        )
        .is_err()
    );
    let ungranted_fixture = Fixture::local(source, fixtures::BUILD, TargetProfile::WindowsX64);
    assert!(
        project_checked_selected_provider_policy(
            &ungranted_fixture
                .changed_build_target("package", "accept_boundary#windows_x86_64::satisfies::Host"),
            ungranted_fixture.target,
            package_identity(),
        )
        .is_err(),
        "an initially empty grant collection must still reharvest the authoritative build"
    );
    let ungranted = project(&ungranted_fixture);
    assert!(ungranted.plans()[0].grants().is_empty());
    assert_ne!(policy, ungranted);
    assert_ne!(
        policy.canonical_bytes().unwrap(),
        ungranted.canonical_bytes().unwrap()
    );
}

#[test]
fn atomic_family_coordinates_rejoin_complete_normalized_plans() {
    let fixture = Fixture::local(
        fixtures::FAMILY,
        fixtures::FAMILY_BUILD,
        TargetProfile::WindowsX64,
    );
    let policy = project(&fixture);
    let [family] = policy.families() else {
        panic!("one atomic family")
    };
    assert_eq!(family.family_identity().path(), "CheckedMath::convert");
    assert_eq!(family.provider_type_declaration().path(), "ConvertProvider");
    assert_eq!(
        family.authority(),
        PackageReviewProviderSelectionAuthority::BuildOverride
    );
    assert_eq!(
        family.coverage(),
        PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily
    );
    assert_eq!(family.coordinates().len(), 2);
    assert_ne!(
        family.coordinates()[0].plan_index(),
        family.coordinates()[1].plan_index()
    );
    for coordinate in family.coordinates() {
        let plan = &policy.plans()[coordinate.plan_index() as usize];
        assert_eq!(
            plan.provider_type_declaration(),
            Some(family.provider_type_declaration())
        );
        assert!(
            plan.rows()
                .iter()
                .any(|row| row.requirement().path() == coordinate.requirement_identity())
        );
        assert!(plan.rows().iter().all(|row| matches!(
            row.binding(),
            PackagePolicyProviderBinding::CheckedAdapter { .. }
        )));
    }
    assert!(
        project_checked_selected_provider_policy(
            &fixture.changed_build_target("select_provider", "ignored_selection"),
            fixture.target,
            package_identity(),
        )
        .is_err()
    );
}

#[test]
fn generic_checked_adapter_policy_does_not_persist_specialization_names() {
    let fixture = Fixture::local(
        fixtures::GENERIC,
        fixtures::BUILD,
        TargetProfile::WindowsX64,
    );
    assert!(fixture.checked.typed.machine_specializations.len() >= 2);
    let policy = project(&fixture);
    let plan = policy
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "GenericMath::identity")
        .unwrap();
    let [row] = plan.rows() else {
        panic!("one declared generic adapter selection")
    };
    assert_eq!(row.realization().path(), "GenericProvider::identity");
    assert!(matches!(
        row.binding(),
        PackagePolicyProviderBinding::CheckedAdapter { .. }
    ));
    let bytes = policy.canonical_bytes().unwrap();
    assert!(
        !bytes
            .windows(b"$specialized$".len())
            .any(|window| window == b"$specialized$")
    );
    assert!(
        project_checked_selected_provider_policy(
            &fixture.changed_build_target("package", "select_provider"),
            fixture.target,
            package_identity(),
        )
        .is_err(),
        "an initially empty authored selection collection must still be reharvested"
    );
}

#[test]
fn explicit_generic_family_retains_declaration_coverage_without_actual_applications() {
    let source = fixtures::GENERIC
        .split("pub machine exercise_i32")
        .next()
        .unwrap();
    let build = fixtures::FAMILY_BUILD
        .replace("CheckedMath::convert", "GenericMath::identity")
        .replace("ConvertProvider", "GenericProvider");
    let fixture = Fixture::local(source, &build, TargetProfile::WindowsX64);
    assert!(fixture.checked.typed.machine_specializations.is_empty());
    assert!(
        fixture
            .checked
            .facts
            .operators
            .boundary_applications
            .is_empty()
    );
    let policy = project(&fixture);
    let [family] = policy.families() else {
        panic!("one generic declaration family")
    };
    assert_eq!(family.family_identity().path(), "GenericMath::identity");
    assert_eq!(
        family.coverage(),
        PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily
    );
    let [coordinate] = family.coordinates() else {
        panic!("one declared generic overload coordinate")
    };
    let plan = &policy.plans()[coordinate.plan_index() as usize];
    assert_eq!(plan.schema_declaration().path(), "GenericMath::identity");
    assert_eq!(
        plan.rows()[0].realization().path(),
        "GenericProvider::identity"
    );
}
