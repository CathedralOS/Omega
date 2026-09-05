use super::*;
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use package_manager::review::{
    ConsumerScopedSemanticBindingReviewInput,
    compile_resolved_package_reviews_with_semantic_bindings,
};

const FILESYSTEM: &str = r#"pub boundary trait FilesystemHost {
    machine inspect() reaches FilesystemHost;
}
pub machine access()
    reaches FilesystemHost
    invokes FilesystemHost;
{
    FilesystemHost::inspect();
}
"#;

fn with_permission(
    tree: &Tree,
    label: &str,
    classes: &[TerminalAuthorityClass],
) -> (ResolvedPackageSourceClosure, CompilerIssuedPackageReviewSet) {
    let closure = resolve(tree, label);
    let target = closure.for_exact_target(TARGET);
    let preliminary =
        compile_resolved_package_reviews(&target, &tree.path(&format!("{label}-preliminary")))
            .unwrap();
    let review = preliminary.review(closure.graph().root()).unwrap();
    let [candidate] = review.semantic_binding_candidates() else {
        panic!("one exact checked filesystem service candidate");
    };
    let [requirement] = candidate.service_schema().methods.as_slice() else {
        panic!("the fixture has one exact permission target");
    };
    let binding = candidate
        .binding()
        .clone()
        .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
            candidate.binding().normalized_schema_digest(),
            requirement.requirement_identity.clone(),
            TerminalAuthorityDisposition::from_classes(classes.iter().copied()),
        )])
        .unwrap();
    let reviews = compile_resolved_package_reviews_with_semantic_bindings(
        &target,
        &tree.path(&format!("{label}-final")),
        &[ConsumerScopedSemanticBindingReviewInput::new(
            closure.graph().root().clone(),
            binding,
        )],
    )
    .unwrap();
    (closure, reviews)
}

#[test]
fn exact_permission_and_danger_require_initial_decisions_and_retained_danger_recommends_audit() {
    let tree = Tree::new();
    source(&tree, FILESYSTEM, "");
    let (closure, reviews) = with_permission(
        &tree,
        "accepted-authority",
        &[TerminalAuthorityClass::FilesystemContentRead],
    );
    let policy = reviews.review(closure.graph().root()).unwrap().policy();
    assert!(policy.selected_providers().plans().is_empty());
    assert_eq!(policy.terminal_permissions().services().len(), 1);
    assert_eq!(
        policy.terminal_permissions().services()[0]
            .permissions()
            .len(),
        1
    );
    let initial = compare_package_policy_changes(
        None,
        &reviews,
        &closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    let [package] = initial.packages() else {
        panic!("one authority package")
    };
    for kind in [
        PackagePolicyRowKind::TerminalPermission,
        PackagePolicyRowKind::DangerousCapability,
    ] {
        let row = package
            .rows()
            .iter()
            .find(|row| row.kind() == kind)
            .unwrap();
        assert!(row.requires_decision());
        if kind == PackagePolicyRowKind::DangerousCapability {
            assert!(row.audit_recommended());
        }
    }
    assert!(package.audit_recommended());
    let lock = lock_from_reviews(&closure, &reviews);
    source(
        &tree,
        &format!("// implementation-only revision\n{FILESYSTEM}"),
        "",
    );
    let (updated_sources, updated_reviews) = with_permission(
        &tree,
        "updated-authority",
        &[TerminalAuthorityClass::FilesystemContentRead],
    );
    let updated = compare_package_policy_changes(
        lock.target(TARGET),
        &updated_reviews,
        &updated_sources.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    let [package] = updated.packages() else {
        panic!("one updated package")
    };
    assert!(package.source_changed());
    assert!(package.rows().is_empty());
    assert!(!package.requires_decision());
    assert!(package.audit_recommended());

    let (empty_sources, empty_reviews) = with_permission(&tree, "empty-permission", &[]);
    let empty = compare_package_policy_changes(
        None,
        &empty_reviews,
        &empty_sources.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert!(empty.packages()[0].rows().iter().any(|row| {
        row.kind() == PackagePolicyRowKind::TerminalPermission && row.requires_decision()
    }));
}
