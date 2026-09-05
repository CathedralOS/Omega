use super::*;

const GENERIC: &str = r#"
pub boundary trait FilesystemHost<'scope, Element, const Count: u64> {
    machine read(value: &'scope [Element; Count]) -> u64;
    machine visit<machine Work>(value: &'scope Element) -> u64
    where machine Work(input: Element) -> u64;
    ;
}
"#;

#[test]
fn unused_generic_service_retains_type_const_lifetime_and_machine_contracts() {
    let fixture = fixtures::Fixture::filesystem(GENERIC, false, "FilesystemHost", "read");
    let checked = fixture.check(Some(read_permission()));
    assert!(checked.selected_provider_plans().plans().is_empty());
    let policy = project(&checked, fixture.target);
    let service = &policy.services()[0];
    assert_eq!(service.lifetime_parameter_count(), 1);
    assert_eq!(service.static_parameters().len(), 2);
    assert!(matches!(
        service.static_parameters()[0].kind(),
        PackageReviewTypeParameterKind::Type
    ));
    assert!(
        matches!(service.static_parameters()[1].kind(), PackageReviewTypeParameterKind::Const(carrier) if carrier.canonical().contains("u64"))
    );
    let read = service
        .methods()
        .iter()
        .find(|method| method.name() == "read")
        .unwrap();
    assert_eq!(read.signature().schema_lifetime_parameter_count(), 1);
    assert_eq!(read.signature().schema_arguments().len(), 2);
    assert_eq!(read.signature().requirement_arguments().len(), 2);
    assert_eq!(read.signature().requirement_lifetime_arguments(), &[0]);
    let visit = service
        .methods()
        .iter()
        .find(|method| method.name() == "visit")
        .unwrap();
    assert!(matches!(
        visit.signature().static_parameters()[0].kind(),
        PackageReviewTypeParameterKind::Machine(_)
    ));
    assert!(
        service
            .methods()
            .iter()
            .all(|method| method.calling().is_none())
    );

    let renamed_source = GENERIC
        .replace("'scope", "'borrow")
        .replace("Element", "Item")
        .replace("Count", "Width")
        .replace("Work", "Operation");
    let renamed = fixtures::Fixture::filesystem(&renamed_source, false, "FilesystemHost", "read");
    let renamed = project(&renamed.check(Some(read_permission())), renamed.target);
    assert_eq!(
        service.static_parameters(),
        renamed.services()[0].static_parameters()
    );
    for (original, renamed) in service
        .methods()
        .iter()
        .zip(renamed.services()[0].methods())
    {
        assert_eq!(
            original.signature(),
            renamed.signature(),
            "binder spelling does not change the typed signature"
        );
    }
    assert_eq!(
        policy, renamed,
        "binder spelling does not change normalized terminal policy"
    );
    assert_eq!(
        policy.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );

    let changed_source = GENERIC.replace("[Element; Count]", "[Element; 7]");
    let changed = fixtures::Fixture::filesystem(&changed_source, false, "FilesystemHost", "read");
    let changed = project(&changed.check(Some(read_permission())), changed.target);
    assert_eq!(
        service.permissions()[0].permitted(),
        changed.services()[0].permissions()[0].permitted()
    );
    assert_ne!(
        service.permissions()[0].requirement(),
        changed.services()[0].permissions()[0].requirement()
    );
    assert_eq!(
        changed.services()[0].permissions()[0].requirement(),
        changed.services()[0]
            .methods()
            .iter()
            .find(|method| method.name() == "read")
            .unwrap()
            .requirement()
    );
    assert_ne!(
        read.signature(),
        changed.services()[0]
            .methods()
            .iter()
            .find(|method| method.name() == "read")
            .unwrap()
            .signature()
    );
    assert_ne!(
        policy.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}

#[test]
fn inherited_generic_service_retains_selected_and_declaring_owner_and_argument_relations() {
    let source = r#"
pub boundary trait FilesystemBase<'scope, Element, const Count: u64> {
    machine read(value: &'scope [Element; Count]) -> u64;
}
pub boundary trait FilesystemHost<'borrow, Item, const Width: u64, const Alternate: u64>:
    FilesystemBase<'borrow, Item, Width> {}
"#;
    let fixture = fixtures::Fixture::filesystem(source, true, "FilesystemHost", "read");
    let policy = project(&fixture.check(Some(read_permission())), fixture.target);
    let service = &policy.services()[0];
    assert_eq!(service.service().path(), "FilesystemHost");
    assert_eq!(
        service.service().owner(),
        PackageReviewNominalOwner::Package(fixture.owner)
    );
    assert_eq!(service.lifetime_parameter_count(), 1);
    assert_eq!(service.static_parameters().len(), 3);
    let method = &service.methods()[0];
    assert_eq!(method.requirement_owner().path(), "FilesystemBase");
    assert_eq!(
        method.requirement_owner().owner(),
        PackageReviewNominalOwner::Package(fixture.owner)
    );
    assert_eq!(service.permissions()[0].requirement(), method.requirement());
    assert_eq!(
        &method.signature().schema_arguments()[..2],
        method.signature().requirement_arguments()
    );
    assert_eq!(method.signature().requirement_lifetime_arguments(), &[0]);

    let changed_source = source.replace(
        "FilesystemBase<'borrow, Item, Width>",
        "FilesystemBase<'borrow, Item, Alternate>",
    );
    let changed = fixtures::Fixture::filesystem(&changed_source, true, "FilesystemHost", "read");
    let changed = project(&changed.check(Some(read_permission())), changed.target);
    assert_eq!(
        service.static_parameters(),
        changed.services()[0].static_parameters()
    );
    assert_eq!(
        method.signature().schema_arguments(),
        changed.services()[0].methods()[0]
            .signature()
            .schema_arguments()
    );
    assert_eq!(
        &changed.services()[0].methods()[0]
            .signature()
            .requirement_arguments()[1],
        &changed.services()[0].methods()[0]
            .signature()
            .schema_arguments()[2]
    );
    assert_ne!(
        method.signature().requirement_arguments(),
        changed.services()[0].methods()[0]
            .signature()
            .requirement_arguments()
    );
    assert_ne!(
        method.signature().parameters(),
        changed.services()[0].methods()[0].signature().parameters()
    );
    assert_ne!(
        policy.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}
