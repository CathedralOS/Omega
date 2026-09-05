use super::*;

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
