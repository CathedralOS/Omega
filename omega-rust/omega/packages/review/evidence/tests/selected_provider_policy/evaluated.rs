use super::*;

#[test]
fn normalized_import_preserves_meaning_while_evaluation_receipts_change() {
    let source = format!(
        "{}{}",
        fixtures::import_producer(false, "ExitProcess"),
        fixtures::IMPORT_LEAF
    );
    let first = Fixture::local(&source, fixtures::BUILD, TargetProfile::WindowsX64);
    let indirect_source = format!(
        "{}{}",
        fixtures::import_producer(true, "ExitProcess"),
        fixtures::IMPORT_LEAF
    );
    let indirect = Fixture::local(&indirect_source, fixtures::BUILD, TargetProfile::WindowsX64);
    let policy = project(&first);
    let indirect_policy = project(&indirect);
    assert_eq!(
        policy, indirect_policy,
        "equivalent producer implementation is not selected binding meaning"
    );
    assert_eq!(
        policy.canonical_bytes().unwrap(),
        indirect_policy.canonical_bytes().unwrap()
    );
    assert_ne!(
        first.checked.evaluated_via_bindings().rows()[0]
            .evaluated()
            .receipt(),
        indirect.checked.evaluated_via_bindings().rows()[0]
            .evaluated()
            .receipt(),
        "the source change must exercise actual receipt exclusion",
    );
    let plan = policy
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "Host")
        .unwrap();
    let [row] = plan.rows() else {
        panic!("one selected normalized import")
    };
    let PackagePolicyProviderBinding::Import {
        target,
        locator,
        producer,
    } = row.binding()
    else {
        panic!("ordinary evaluated import must remain distinct from bootstrap strings")
    };
    assert_eq!(target, "omega.target-profile.v1:windows_x86_64");
    assert_eq!(
        locator,
        &PackageReviewForeignLocator::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        }
    );
    assert_eq!(producer.declaration().path(), "import_binding");
    assert_eq!(
        producer.declaration().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(producer.package(), Some(package_identity()));
    assert!(producer.callable_identity().contains("import_binding"));
    assert_eq!(row.realization().path(), "ping_leaf");
    assert!(
        project_checked_selected_provider_policy(
            &first.without_typed_via("ping_leaf"),
            first.target,
            package_identity(),
        )
        .is_err()
    );

    let changed_source = format!(
        "{}{}",
        fixtures::import_producer(false, "FreeLibrary"),
        fixtures::IMPORT_LEAF
    );
    let changed = project(&Fixture::local(
        &changed_source,
        fixtures::BUILD,
        TargetProfile::WindowsX64,
    ));
    assert_ne!(policy, changed);
    assert_ne!(
        policy.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}

#[test]
fn selected_import_keeps_foreign_producer_owner_separate_from_local_leaf() {
    let source = format!("use producer::bindings;\n{}", fixtures::IMPORT_LEAF);
    let fixture = Fixture::foreign(
        &source,
        &fixtures::import_producer(false, "ExitProcess"),
        TargetProfile::WindowsX64,
    );
    let policy = project(&fixture);
    let plan = policy
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "Host")
        .unwrap();
    let row = &plan.rows()[0];
    let PackagePolicyProviderBinding::Import { producer, .. } = row.binding() else {
        panic!("evaluated import")
    };
    assert_eq!(producer.package(), Some(fixtures::foreign_identity()));
    assert_eq!(
        producer.declaration().owner(),
        PackageReviewNominalOwner::Package(fixtures::foreign_identity())
    );
    assert_eq!(producer.declaration().path(), "import_binding");
    assert_eq!(
        row.realization().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(plan.schema_declaration().owner(), row.realization().owner());
}

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
