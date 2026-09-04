use crate::support::*;

fn compile_selected_reach_fixture() -> omega_compiler::CheckedCompilation {
    let target =
        host_target_name().expect("package evidence tests require a supported host target");
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait MachineControl { }
pub boundary trait PortIo { }

pub boundary trait InterruptCompletion {
    machine complete() -> u64
    reaches <= MachineControl + PortIo;
}

data Pic { }

machine Pic::complete() -> u64
satisfies InterruptCompletion::complete
reaches PortIo
{
    0
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("selected installation-reach fixture should check")
}

#[test]
fn selected_provider_projects_exact_installation_reach_refinement() {
    let checked = compile_selected_reach_fixture();
    let review = project_checked_package_review(&checked)
        .expect("selected installation reach should project");
    let provider = review
        .selected_providers()
        .iter()
        .find(|provider| provider.service_schema() == "InterruptCompletion")
        .expect("interrupt completion provider");
    let row = provider
        .row_declarations()
        .iter()
        .find(|row| {
            row.requirement()
                .path()
                .contains("InterruptCompletion::complete")
        })
        .expect("exact interrupt completion requirement");
    let reach = row
        .installation_reach()
        .expect("installation-bound row retains its selected reach");

    assert_eq!(
        reach
            .upper_bound()
            .iter()
            .map(|service| service.path())
            .collect::<Vec<_>>(),
        ["MachineControl", "PortIo"],
    );
    assert_eq!(
        reach
            .resolved()
            .iter()
            .map(|service| service.path())
            .collect::<Vec<_>>(),
        ["PortIo"],
    );
    assert!(
        reach.upper_bound().iter().chain(reach.resolved()).all(
            |service| service.owner() == PackageReviewNominalOwner::Package(package_identity())
        ),
        "selected reach must retain exact package-qualified service identity",
    );
    assert!(
        review
            .canonical_rows()
            .expect("selected reach is canonically encodable")
            .iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet),
    );
}

#[test]
fn selected_provider_rejects_checked_installation_reach_drift() {
    let mut checked = compile_selected_reach_fixture();
    let realization = checked
        .machines()
        .iter()
        .find(|machine| {
            checked
                .normalized_machine_overload_identity(machine)
                .is_some_and(|identity| identity.identity().contains("Pic::complete"))
        })
        .expect("PIC realization")
        .symbol;
    let machine_control = checked
        .facts
        .service_reaches
        .services
        .id_for_name("MachineControl")
        .expect("machine-control service");
    let drifted = checked
        .facts
        .service_reaches
        .rows
        .intern(vec![machine_control]);
    let reach_fact = checked
        .facts
        .service_reaches
        .machines
        .iter()
        .find_map(|(handle, fact)| (fact.machine == realization).then_some(handle))
        .expect("PIC service-reach fact");
    checked
        .facts
        .service_reaches
        .machines
        .get_mut(reach_fact)
        .effective = drifted;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check selected reach drift must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("resolved reach that disagrees with its exact checked realization")),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}
