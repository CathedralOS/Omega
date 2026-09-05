use super::*;

#[test]
fn complete_baseline_joins_nonempty_external_supply_and_selected_provider_meaning() {
    let source = r#"use omega::language::core::external_binding;
pub windows_x86_64 machine import_binding() -> Binding<12, 11, 0> {
    Binding::DllImport { import: DllImport::PeByName { library: "kernel32.dll", export: "ExitProcess" } }
}
pub boundary trait Host { machine ping(); }
pub machine ping_leaf() satisfies Host::ping via import_binding();
"#;
    let fixture = Fixture::local(source);
    let policy = project(&fixture);
    assert!(!policy.selected_providers().plans().is_empty());
    let [supply] = policy.external_supplies() else {
        panic!("one normalized external supply")
    };
    assert!(matches!(
        supply.binding(),
        PackagePolicyExternalBinding::NormalizedImport { .. }
    ));
    let machine = fixture
        .checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ping_leaf")
        .unwrap();
    let expected = omega_package_evidence::project_checked_external_supply_policy(
        &fixture.checked,
        machine.symbol,
    )
    .unwrap();
    assert_eq!(policy.external_supplies(), expected);
    assert_eq!(
        policy.selected_providers(),
        &project_checked_selected_provider_policy(
            &fixture.checked,
            fixture.target,
            package_identity()
        )
        .unwrap()
    );
    let changed = project(&Fixture::local(
        &source.replace("ExitProcess", "FreeLibrary"),
    ));
    assert_ne!(policy.external_supplies(), changed.external_supplies());
    assert_ne!(policy.selected_providers(), changed.selected_providers());
    assert_ne!(
        policy.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}

#[test]
fn unused_representation_selection_and_terminal_permission_survive_full_assembly() {
    let source = r#"use omega::language::core::representation;
pub boundary data Token;
pub data Carrier { value: u64; }
pub TokenRepresentation: Carrier satisfies OpaqueRepresentation<Token>;
pub boundary trait FilesystemHost {
    machine read(descriptor: i32) -> i64;
    machine stat(descriptor: i32) -> i64;
}
"#;
    let build = r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_representation<Token, TokenRepresentation>();
}
"#;
    let mut fixture = Fixture::with_build(source, build);
    let absent = project(&fixture);
    fixture.grant_filesystem_read();
    let policy = project(&fixture);
    assert!(policy.selected_providers().plans().is_empty());
    assert!(policy.representation().demands().is_empty());
    assert_eq!(policy.representation().selected_availability().len(), 1);
    assert_eq!(policy.representation().producer_availability().len(), 1);
    let [service] = policy.terminal_permissions().services() else {
        panic!("unused permitted service retained")
    };
    assert_eq!(service.service().path(), "FilesystemHost");
    assert_eq!(
        service.methods().len(),
        2,
        "unpermitted sibling remains part of schema"
    );
    assert_eq!(service.permissions().len(), 1);
    assert_eq!(
        policy.representation(),
        &project_checked_representation_policy(&fixture.checked, package_identity()).unwrap()
    );
    assert_eq!(
        policy.terminal_permissions(),
        &project_checked_terminal_permission_policy(
            &fixture.checked,
            fixture.target,
            package_identity()
        )
        .unwrap()
    );
    assert!(absent.terminal_permissions().services().is_empty());
    assert_ne!(
        absent.canonical_bytes().unwrap(),
        policy.canonical_bytes().unwrap()
    );
}
