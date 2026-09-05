use super::*;

#[test]
fn ordinary_uefi_permission_retains_calling_meaning_omitted_from_accepted_schema_digest() {
    let root = TempPackage::new();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap();
    let standard_library = repository.join("source/library/std");
    let standard_package = PackageKeyIdentity::from_digest([54; 32]).unwrap();
    root.write(
        "main.omg",
        r#"use omega::language::core::extent;
use ordinary_std::targets::uefi_x86_64;
data Boot { launch_count: u64; }
machine Boot::launch(&mut self, image: Extent in Granted, initial_storage: Extent in Granted) {
    transition { _ -> retain(image as Extent, initial_storage as Extent) }
    state retain(&mut self, image: Extent, initial_storage: Extent) {
        transition { _ -> retain(image, initial_storage) }
    }
}
"#,
    );
    root.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("review-fixture");
    builder.subsystem = Subsystem::EfiApplication;
    builder.freestanding = true;
    builder.roots.bind(uefi_x86_64::ProgramEntry, Boot::launch);
}
"#,
    );
    let inputs = PackageCompilationInputs::new(
        package_identity(),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(package_identity(), "review-fixture", root.0.clone()),
            PackageSourceBinding::new(standard_package, "ordinary-std", standard_library),
        ],
        vec![PackageDependencyBinding::new(
            package_identity(),
            "ordinary_std",
            standard_package,
        )],
    )
    .unwrap();
    let candidate =
        compile_to_checked_with_packages(&root.0.join("main.omg"), None, inputs.clone())
            .expect("ordinary UEFI semantic-only candidate");
    let role = AcceptedSemanticBindingRole::UefiX64ProgramEntry;
    let binding = candidate
        .candidate_service_binding(role, standard_package, "UefiApplication")
        .unwrap();
    let definition = candidate
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "UefiApplication")
        .unwrap();
    let schema =
        omega_effects::provider_plan::ServiceSchema::from_typed(&candidate.typed, definition)
            .unwrap();
    let physical = schema
        .methods
        .iter()
        .find(|method| method.requirement_owner == "UefiPhysicalEntry")
        .unwrap();
    let binding = binding
        .clone()
        .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
            binding.normalized_schema_digest(),
            physical.requirement_identity.clone(),
            TerminalAuthorityDisposition::from_classes([]),
        )])
        .unwrap();
    let checked = compile_to_checked_with_packages(
        &root.0.join("main.omg"),
        Some("uefi_x86_64"),
        inputs
            .with_accepted_semantic_bindings(vec![binding.clone()])
            .unwrap(),
    )
    .expect("accepted ordinary UEFI target compilation without native emission");
    let definition = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "UefiApplication")
        .unwrap();
    let schema =
        omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, definition)
            .unwrap();
    assert!(
        schema
            .methods
            .iter()
            .any(|method| method.calling_plan_commitment.is_some())
    );
    assert_eq!(
        omega_package_compilation::accepted_service_schema_digest(role, &schema),
        binding.normalized_schema_digest()
    );
    assert_ne!(schema.identity_digest(), binding.normalized_schema_digest());
    let policy = project(&checked, TargetProfile::UefiX64);
    let [service] = policy.services() else {
        panic!("one accepted UEFI permission service")
    };
    assert_eq!(service.service().path(), "UefiApplication");
    assert_eq!(
        service.service().owner(),
        PackageReviewNominalOwner::Package(standard_package)
    );
    assert_eq!(service.methods().len(), 2);
    assert_eq!(service.permissions().len(), 1);
    for method in service.methods() {
        let calling = method
            .calling()
            .expect("target-closed UEFI inherited method retains its complete calling application");
        assert_eq!(calling.boundary_trait(), service.service());
        assert_eq!(calling.requirement_trait(), method.requirement_owner());
        assert_eq!(calling.requirement(), method.requirement());
        assert_eq!(calling.semantic_parameters().len(), 2);
        assert_eq!(calling.physical().parameters().len(), 2);
    }
    let physical = service
        .methods()
        .iter()
        .find(|method| method.requirement_owner().path() == "UefiPhysicalEntry")
        .unwrap();
    assert_eq!(
        service.permissions()[0].requirement(),
        physical.requirement()
    );
    assert!(service.permissions()[0].permitted().classes().is_empty());
}
