use super::*;
use package_manager::declarations::BuildDeclarationKind;
use package_manager::review::{PackagePolicyChangeError, ReviewOnlyRootRoleContract};

#[test]
fn the_same_root_key_retains_directional_package_application_role_changes() {
    let tree = Tree::new();
    source(
        &tree,
        "data Main { }\nmachine Main::main(&mut self) { }\n",
        "",
    );
    let (package_sources, package_reviews) = candidate(&tree, "package-role");
    let package_lock = lock_from_reviews(&package_sources, &package_reviews);
    fs::write(
        tree.path("sources/root/build.omg"),
        concat!(
            "machine build(builder: &mut Build) {\n",
            " builder.application(\"policy-fixture\");\n",
            " builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);\n",
            "}\n",
        ),
    )
    .unwrap();
    let (application_sources, application_reviews) = candidate(&tree, "application-role");
    assert_eq!(
        package_sources.graph().root(),
        application_sources.graph().root()
    );
    let to_application = compare_package_policy_changes(
        package_lock.target(TARGET),
        &application_reviews,
        &application_sources.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert!(to_application.root_changed());
    assert!(to_application.requires_decision());
    let role = to_application.root_role_change().unwrap();
    assert_eq!(role.root(), package_sources.graph().root());
    assert_eq!(role.baseline_role(), BuildDeclarationKind::Package);
    assert_eq!(role.candidate_role(), BuildDeclarationKind::Application);
    assert_eq!(
        role.broken_contract(),
        ReviewOnlyRootRoleContract::DependencyCompatibility
    );
    let application_lock = lock_from_reviews(&application_sources, &application_reviews);
    let to_package = compare_package_policy_changes(
        application_lock.target(TARGET),
        &package_reviews,
        &package_sources.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert!(to_package.root_changed());
    assert!(to_package.requires_decision());
    let role = to_package.root_role_change().unwrap();
    assert_eq!(role.baseline_role(), BuildDeclarationKind::Application);
    assert_eq!(role.candidate_role(), BuildDeclarationKind::Package);
    assert_eq!(
        role.broken_contract(),
        ReviewOnlyRootRoleContract::ApplicationActivation
    );
    assert_ne!(to_application.fingerprint(), to_package.fingerprint());
}

#[test]
fn exact_target_and_current_candidate_source_associations_are_not_policy_deltas() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (closure, reviews) = candidate(&tree, "exact");
    let lock = lock_from_reviews(&closure, &reviews);
    let linux = compile_resolved_package_candidate_reviews(
        &closure.for_exact_target(TargetProfile::LinuxArm64),
        &tree.path("linux-build"),
    )
    .unwrap();
    assert!(matches!(
        compare_package_policy_changes(
            lock.target(TARGET),
            &linux,
            &closure.for_exact_target(TargetProfile::LinuxArm64),
            PackagePolicyChangeLimits::default(),
        ),
        Err(PackagePolicyChangeError::TargetMismatch)
    ));
    source(&tree, "pub const VALUE: u64 = 8;\n", "");
    let different_source = resolve(&tree, "different");
    assert!(matches!(
        compare_package_policy_changes(
            lock.target(TARGET),
            &reviews,
            &different_source.for_exact_target(TARGET),
            PackagePolicyChangeLimits::default(),
        ),
        Err(PackagePolicyChangeError::CandidateReview { .. })
    ));
}

#[test]
fn initial_public_api_is_nonblocking_but_private_assumptions_and_external_code_are_not() {
    for (main, blocking_kind) in [
        ("pub const VALUE: u64 = 7;\n", None),
        (
            "boundary machine trusted_zero() -> u64 ensures result == 0;\n",
            Some(PackagePolicyRowKind::Callable),
        ),
        (
            concat!(
                "pub boundary trait ForeignSurface { machine invoke() reaches ForeignSurface; }\n",
                "pub machine invoke_leaf() satisfies ForeignSurface::invoke\n",
                " via Binding::DllImport(\"omega-host\", \"invoke\");\n",
            ),
            Some(PackagePolicyRowKind::ExternalSupply),
        ),
    ] {
        let tree = Tree::new();
        source(&tree, main, "");
        let (closure, reviews) = candidate(&tree, "initial");
        let changes = compare_package_policy_changes(
            None,
            &reviews,
            &closure.for_exact_target(TARGET),
            PackagePolicyChangeLimits::default(),
        )
        .unwrap();
        assert!(changes.baseline_source_subject().is_none());
        assert_eq!(changes.requires_decision(), blocking_kind.is_some());
        let [package] = changes.packages() else {
            panic!("one initial package")
        };
        assert!(package.baseline_resolution().is_none());
        assert!(
            package
                .rows()
                .iter()
                .all(|row| row.change() == PackagePolicyChangeKind::Added)
        );
        if let Some(kind) = blocking_kind {
            assert!(
                package
                    .rows()
                    .iter()
                    .any(|row| row.kind() == kind && row.requires_decision())
            );
        } else {
            assert!(
                package
                    .rows()
                    .iter()
                    .any(|row| row.kind() == PackagePolicyRowKind::PublicConst)
            );
            assert!(package.rows().iter().all(|row| !row.requires_decision()));
        }
    }
}

#[test]
fn unused_representation_selection_is_retained_as_nonblocking_audit_meaning() {
    let tree = Tree::new();
    source(
        &tree,
        concat!(
            "use omega::language::core::representation;\n",
            "pub boundary data Token;\n",
            "pub data Carrier { value: u64; }\n",
            "pub TokenRepresentation: Carrier satisfies OpaqueRepresentation<Token>;\n",
        ),
        " builder.select_representation<Token, TokenRepresentation>();\n",
    );
    let (closure, reviews) = candidate(&tree, "representation");
    let policy = reviews.review(closure.graph().root()).unwrap().policy();
    assert_eq!(policy.representation().selected_availability().len(), 1);
    assert!(policy.representation().demands().is_empty());
    let changes = compare_package_policy_changes(
        None,
        &reviews,
        &closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    let [package] = changes.packages() else {
        panic!("one representation package")
    };
    let row = package
        .rows()
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::RepresentationSelection)
        .unwrap();
    assert!(!row.requires_decision());
    assert!(row.audit_recommended());
    assert!(package.audit_recommended());
    assert!(
        row.candidate()
            .unwrap()
            .canonical_text()
            .contains("TokenRepresentation")
    );
}
