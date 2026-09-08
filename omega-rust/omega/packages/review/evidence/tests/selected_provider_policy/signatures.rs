use super::*;

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
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("build selection differs from its current authored declaration")),
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

#[test]
fn source_qualified_service_types_do_not_require_a_calling_policy() {
    let carrier = "pub data Carrier { value: u64; }\n";
    let service = r#"pub boundary trait Echo {
    machine echo(value: Carrier) -> Carrier;
}
pub data EchoProvider {}
pub EchoProviderEcho: EchoProvider satisfies Echo;
pub machine EchoProvider::echo(value: Carrier) -> Carrier
satisfies Echo::echo { value }
"#;
    let local = Fixture::local(
        &format!("{carrier}{service}"),
        fixtures::BUILD,
        TargetProfile::WindowsX64,
    );
    let foreign = Fixture::foreign(
        &format!("use producer::bindings;\n{service}"),
        carrier,
        TargetProfile::WindowsX64,
    );
    assert!(
        local
            .checked
            .boundary_calling_plan_realizations()
            .is_empty()
    );
    assert!(
        foreign
            .checked
            .boundary_calling_plan_realizations()
            .is_empty()
    );
    let local = project(&local);
    let foreign = project(&foreign);
    let local_method = &local
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "Echo")
        .unwrap()
        .methods()[0];
    let foreign_method = &foreign
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "Echo")
        .unwrap()
        .methods()[0];
    assert!(local_method.calling().is_none());
    assert!(foreign_method.calling().is_none());
    assert_eq!(
        local_method.parameter_type_identities(),
        foreign_method.parameter_type_identities(),
        "the older local spelling is not sufficient across source owners"
    );
    assert_eq!(
        local_method.result_type_identity(),
        foreign_method.result_type_identity()
    );
    let local_signature = local_method.signature();
    let foreign_signature = foreign_method.signature();
    assert_eq!(local_signature.parameters().len(), 1);
    assert_eq!(foreign_signature.parameters().len(), 1);
    assert_eq!(local_signature.parameters()[0].name(), "value");
    assert_eq!(foreign_signature.parameters()[0].name(), "value");
    assert_eq!(
        local_signature.parameters()[0].type_identity(),
        local_signature.result().unwrap()
    );
    assert_eq!(
        foreign_signature.parameters()[0].type_identity(),
        foreign_signature.result().unwrap()
    );
    assert_ne!(
        local_signature.parameters()[0].type_identity(),
        foreign_signature.parameters()[0].type_identity()
    );
    assert_ne!(local_signature, foreign_signature);
    assert_ne!(local, foreign);
    assert_ne!(
        local.canonical_bytes().unwrap(),
        foreign.canonical_bytes().unwrap()
    );
}
