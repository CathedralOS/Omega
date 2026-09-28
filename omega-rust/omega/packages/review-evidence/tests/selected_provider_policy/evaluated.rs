use super::{
    Fixture, PackagePolicyProviderBinding, TargetProfile, fixtures, package_identity, project,
    project_checked_selected_provider_policy,
};

#[test]
fn evaluated_syscall_retains_producer_instead_of_collapsing_to_raw_number() {
    let fixture = Fixture::local(fixtures::SYSCALL, fixtures::BUILD, TargetProfile::LinuxX64);
    let policy = project(&fixture);
    let plan = policy
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "Process")
        .unwrap();
    let PackagePolicyProviderBinding::Syscall {
        number,
        evaluated: Some(evaluated),
    } = plan.rows()[0].binding()
    else {
        panic!("evaluated syscall must retain its exact producer")
    };
    assert_eq!(*number, 60);
    assert_eq!(evaluated.target(), "omega.target-profile.v1:linux_x86_64");
    assert_eq!(evaluated.producer().declaration().path(), "exit_binding");
    assert_eq!(evaluated.producer().package(), Some(package_identity()));
    assert!(
        project_checked_selected_provider_policy(
            &fixture.without_typed_via("exit_leaf"),
            fixture.target,
            package_identity(),
        )
        .is_err()
    );
}
