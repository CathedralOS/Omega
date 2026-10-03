use super::{
    Fixture, PackagePolicyProviderBinding, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderGrantSelectorKind, PackageReviewProviderSelectionAuthority, TargetProfile,
    fixtures, package_identity, project, project_checked_selected_provider_policy,
};
#[test]
fn unused_selected_plan_retains_its_explicit_grant() {
    let source = r#"boundary trait Host { machine ping(); }
machine ping_leaf() satisfies Host::ping via ForeignBinding::Syscall(60);
"#;
    let granted_build = r#"machine build(builder: &mut Build) {
    Build::package(builder, "review_fixture");
    builder.accept_boundary<windows_x86_64::satisfies::Host>();
}
"#;
    let granted = Fixture::local(source, granted_build, TargetProfile::WindowsX64);
    assert_eq!(
        granted
            .checked
            .custody
            .execution()
            .selected_provider_grants
            .as_slice()
            .len(),
        1
    );
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
    // The toolchain settles entry-boundary calling plans on selected-target
    // compiles; the fixture contract is that the authored boundary trait
    // carries no calling realization of its own.
    let host = granted
        .checked
        .traits()
        .iter()
        .find(|trait_| trait_.name.as_str() == "Host")
        .expect("fixture boundary trait")
        .symbol;
    assert!(
        granted
            .checked
            .custody
            .providers()
            .boundary_calling_plan_realizations
            .iter()
            .all(|realization| realization.boundary_trait != host)
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
    assert_atomic_family_coordinates_rejoin_complete_normalized_plans(fixtures::FAMILY);
}

#[test]
fn token_machine_family_coordinates_rejoin_complete_normalized_plans() {
    assert_atomic_family_coordinates_rejoin_complete_normalized_plans(
        r#"pub data CheckedMath {}
pub boundary machine % CheckedMath::convert(value: u64, divisor: u64) -> u64;
pub boundary machine % CheckedMath::convert(value: i32, divisor: i32) -> i32;
pub data ConvertProvider {}
pub machine ConvertProvider::convert_u64(input: u64, divisor: u64) -> u64
satisfies CheckedMath::convert { input }
pub machine ConvertProvider::convert_i32(input: i32, divisor: i32) -> i32
satisfies CheckedMath::convert { input }
"#,
    );
}

fn assert_atomic_family_coordinates_rejoin_complete_normalized_plans(source: &str) {
    let fixture = Fixture::local(source, fixtures::FAMILY_BUILD, TargetProfile::WindowsX64);
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
