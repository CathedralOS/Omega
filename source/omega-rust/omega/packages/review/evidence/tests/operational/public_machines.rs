use crate::support::*;

#[test]
fn public_machine_visibility_survives_checked_compilation_and_strict_empty_contracts() {
    let package = TempPackage::new();
    package.write("main.omg", "pub machine Package::entry() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("public machine should check");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("checked public machine");
    assert!(machine.is_public);
    assert_eq!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::CheckedBody
    );

    let service = checked
        .facts
        .service_reaches
        .for_machine(machine.symbol)
        .expect("checked service contract");
    assert!(matches!(
        service.interface,
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(_)
    ));
    let invocation = checked
        .facts
        .synchronous_invocations
        .for_machine(machine.symbol)
        .expect("checked invocation contract");
    assert_eq!(
        invocation.interface,
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling
    );
    assert!(matches!(
        checked
            .facts
            .suspensions
            .for_machine(machine.symbol)
            .expect("checked suspension contract")
            .interface,
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(false)
    ));
    assert!(matches!(
        checked
            .facts
            .blocking
            .for_machine(machine.symbol)
            .expect("checked blocking contract")
            .interface,
        psi_language_semantics::BlockingInterface::PublishedMayBlock(false)
    ));
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .expect("checked contract")
            .crash
            .interface(),
        psi_checked_trees::CrashInterface::PublishedCeiling
    );
}

#[test]
fn public_machine_cannot_hide_realized_reach_invocation_or_operational_effects() {
    let cases = [
        (
            "invocation",
            r#"pub boundary trait Handler { machine handle(); }
pub machine public_api(handler: &mut Handler) { handler.handle(); }
"#,
            &["omits `invokes handler;`"][..],
        ),
        (
            "operational",
            r#"pub boundary trait Waiting { machine wait() reaches Waiting suspends; blocks; }
pub machine public_api(waiting: &mut Waiting)
reaches Waiting
invokes waiting;
{
    suspend block waiting.wait();
}
"#,
            &["omits `suspends;`", "omits `blocks;`"][..],
        ),
        (
            "crash",
            r#"pub machine public_api() { crash Abort; }
"#,
            &["crash"][..],
        ),
    ];

    for (label, source, expected_messages) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            None,
            package_inputs(&package.0),
        )
        .unwrap_err();
        for expected in expected_messages {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{label} omission should mention `{expected}`: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn exact_synchronous_invocations_change_comparison_encoding() {
    let quiet = TempPackage::new();
    let invoking = TempPackage::new();
    quiet.write(
        "main.omg",
        r#"pub boundary trait Handler { machine handle(); }
pub boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
reaches Host
invokes handler;
invokes Host;
{ }
"#,
    );
    invoking.write(
        "main.omg",
        r#"pub boundary trait Handler { machine handle(); }
pub boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
invokes handler;
invokes Host;
{
    handler.handle();
    Host::ping();
}
"#,
    );
    let build = r#"target windows_x86_64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    quiet.write("build.omg", build);
    invoking.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("invocation comparison fixture should check")
    };
    let quiet = project_checked_package_review(&compile(&quiet)).expect("quiet review");
    let invoking = project_checked_package_review(&compile(&invoking)).expect("invoking review");
    let dispatch = invoking
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public dispatch row");
    let quiet_dispatch = quiet
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("quiet public dispatch row");
    let declared = dispatch
        .declared_synchronous_invocations()
        .expect("published invocation ceiling");
    assert_eq!(declared.len(), 2);
    assert_eq!(
        declared[0],
        PackageReviewSynchronousInvocation::Parameter(0)
    );
    let PackageReviewSynchronousInvocation::Service(service) = &declared[1] else {
        panic!("second exact invocation should be a service identity")
    };
    assert_eq!(service.path(), "Host");
    assert_eq!(
        service.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        quiet_dispatch.declared_synchronous_invocations(),
        Some(declared)
    );
    assert!(quiet_dispatch.realized_synchronous_invocations().is_empty());
    assert_eq!(quiet_dispatch.contracts(), dispatch.contracts());
    assert_eq!(dispatch.realized_synchronous_invocations(), declared);
    assert_ne!(
        quiet.canonical_review_bytes().expect("quiet encoding"),
        invoking
            .canonical_review_bytes()
            .expect("invoking encoding")
    );
}
