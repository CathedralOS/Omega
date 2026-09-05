use super::*;

#[test]
fn selected_installation_ceiling_and_checked_refinement_are_policy() {
    let source = r#"pub boundary trait MachineControl {}
pub boundary trait PortIo {}
pub boundary trait InterruptCompletion {
    machine complete() -> u64 reaches <= MachineControl + PortIo;
}
data Pic {}
machine Pic::complete() -> u64
satisfies InterruptCompletion::complete
reaches PortIo
{ 0 }
"#;
    let fixture = Fixture::local(source, fixtures::BUILD, TargetProfile::WindowsX64);
    let policy = project(&fixture);
    let plan = policy
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "InterruptCompletion")
        .unwrap();
    let reach = plan.rows()[0]
        .installation_reach()
        .expect("selected installation refinement");
    assert_eq!(
        reach
            .upper_bound()
            .iter()
            .map(|service| service.path())
            .collect::<Vec<_>>(),
        ["MachineControl", "PortIo"]
    );
    assert_eq!(
        reach
            .resolved()
            .iter()
            .map(|service| service.path())
            .collect::<Vec<_>>(),
        ["PortIo"]
    );
    assert!(
        reach.upper_bound().iter().chain(reach.resolved()).all(
            |service| service.owner() == PackageReviewNominalOwner::Package(package_identity())
        )
    );
    let mut changed = fixture.checked.clone();
    let realization = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Pic::complete")
        .unwrap()
        .symbol;
    let machine_control = changed
        .facts
        .service_reaches
        .services
        .id_for_name("MachineControl")
        .unwrap();
    let row = changed
        .facts
        .service_reaches
        .rows
        .intern(vec![machine_control]);
    let fact = changed
        .facts
        .service_reaches
        .machines
        .iter()
        .find_map(|(handle, fact)| (fact.machine == realization).then_some(handle))
        .unwrap();
    changed
        .facts
        .service_reaches
        .machines
        .get_mut(fact)
        .effective = row;
    assert!(
        project_checked_selected_provider_policy(&changed, fixture.target, package_identity())
            .is_err()
    );
}
