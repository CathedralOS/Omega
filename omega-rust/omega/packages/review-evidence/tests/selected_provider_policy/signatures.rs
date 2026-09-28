use super::{
    Fixture, PackagePolicyMachineParameterContract, PackagePolicyTypeParameterKind, TargetProfile,
    fixtures, package_identity, project, project_checked_selected_provider_policy,
};
#[test]
fn operator_service_signatures_retain_empty_and_generic_static_telescopes() {
    for (source, build, path, static_count, plan_count) in [
        (
            fixtures::FAMILY,
            fixtures::FAMILY_BUILD,
            "CheckedMath::convert",
            0,
            2,
        ),
        (
            fixtures::GENERIC,
            fixtures::BUILD,
            "GenericMath::identity",
            1,
            1,
        ),
    ] {
        let fixture = Fixture::local(source, build, TargetProfile::WindowsX64);
        let policy = project(&fixture);
        let plans = policy
            .plans()
            .iter()
            .filter(|plan| plan.schema_declaration().path() == path)
            .collect::<Vec<_>>();
        assert_eq!(plans.len(), plan_count);
        for plan in plans {
            let [method] = plan.methods() else {
                panic!("one exact operator service signature")
            };
            assert!(method.calling().is_none());
            let signature = method.signature();
            assert_eq!(signature.static_parameters().len(), static_count);
            assert!(signature.schema_arguments().is_empty());
            assert!(signature.requirement_arguments().is_empty());
            assert!(signature.requirement_lifetime_arguments().is_empty());
            assert_eq!(signature.schema_lifetime_parameter_count(), 0);
            assert_eq!(signature.requirement_lifetime_parameter_count(), 0);
            let [parameter] = signature.parameters() else {
                panic!("one source value parameter")
            };
            assert_eq!(parameter.name(), "value");
            assert!(!parameter.is_const());
            assert!(!parameter.is_mutable());
            assert!(!parameter.is_self());
            assert_eq!(signature.result(), Some(parameter.type_identity()));
        }
        assert_eq!(project(&fixture), policy);
    }
}

#[test]
fn empty_static_service_rejects_changed_authored_lifetime_binders() {
    let mut fixture = Fixture::local(
        fixtures::FAMILY,
        fixtures::FAMILY_BUILD,
        TargetProfile::WindowsX64,
    );
    let policy = project(&fixture);
    let plan = policy
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "CheckedMath::convert")
        .expect("selected operator");
    let [method] = plan.methods() else {
        panic!("one operator method")
    };
    assert!(method.calling().is_none());
    let signature = method.signature();
    assert!(signature.static_parameters().is_empty());
    assert_eq!(signature.requirement_lifetime_parameter_count(), 0);

    let operators = fixture.checked.typed.roots.operators;
    let operator = fixture
        .checked
        .typed
        .tables
        .operators
        .span_mut_or_empty(operators)
        .iter_mut()
        .find(|operator| operator.is_boundary)
        .expect("source operator");
    operator.lifetime_parameters = vec!["input".into(), "input".into()];
    let diagnostics = project_checked_selected_provider_policy(
        &fixture.checked,
        fixture.target,
        package_identity(),
    )
    .expect_err("changed authored lifetime binders must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("static-telescope arity")),
        "{diagnostics:#?}"
    );
}

#[test]
fn service_nested_machine_signature_retains_result_absence_without_calling() {
    let source = r#"
pub boundary trait Echo {
    machine echo<machine Work>() -> u64
    where machine Work();
    ;
}
pub data EchoProvider {}
pub EchoProviderEcho: EchoProvider satisfies Echo;
pub machine EchoProvider::echo<machine Work>() -> u64
where machine Work();
satisfies Echo::echo { 0 }
"#;
    let absent = project(&Fixture::local(
        source,
        fixtures::BUILD,
        TargetProfile::WindowsX64,
    ));
    let present = project(&Fixture::local(
        &source.replace("Work();", "Work() -> u64;"),
        fixtures::BUILD,
        TargetProfile::WindowsX64,
    ));
    for (policy, has_result) in [(&absent, false), (&present, true)] {
        let method = &policy
            .plans()
            .iter()
            .find(|plan| plan.schema_declaration().path() == "Echo")
            .unwrap()
            .methods()[0];
        assert!(method.calling().is_none());
        let PackagePolicyTypeParameterKind::Machine(
            PackagePolicyMachineParameterContract::Structural(signature),
        ) = method.signature().static_parameters()[0].kind()
        else {
            panic!("one structural machine parameter")
        };
        assert_eq!(signature.return_type().is_some(), has_result);
    }
    assert_ne!(
        absent.canonical_bytes().unwrap(),
        present.canonical_bytes().unwrap()
    );
}
