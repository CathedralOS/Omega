use super::super::*;
use super::support::*;
use crate::resolution::graph::PackageClosureValidationError;
use std::collections::BTreeMap;

#[test]
fn rejects_dependency_cycle_after_bounded_traversal() {
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("library")],
    );
    let root_again = root.clone();
    let library = custody(
        "library",
        "library",
        2,
        "/snapshots/library",
        vec![request("root-again")],
    );

    let error = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([
            ("library", library),
            ("root-again", root_again),
        ])),
    )
    .expect_err("cycle rejects");

    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::InvalidClosure { ref errors }
            if errors.iter().any(|error| matches!(
                error,
                PackageClosureValidationError::DependencyCycle { .. }
            ))
    ));
}

#[test]
fn enforces_package_request_and_depth_ceilings() {
    let leaf = custody("leaf", "leaf", 3, "/snapshots/leaf", vec![]);
    let middle = custody(
        "middle",
        "middle",
        2,
        "/snapshots/middle",
        vec![request("leaf")],
    );
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("middle")],
    );
    let packages = BTreeMap::from([("middle", middle), ("leaf", leaf)]);

    for (limits, expected_kind) in [
        (
            PackageSourceClosureLimits {
                max_packages: 1,
                max_dependency_requests: 8,
                max_depth: 8,
            },
            PackageSourceClosureLimitKind::Packages,
        ),
        (
            PackageSourceClosureLimits {
                max_packages: 8,
                max_dependency_requests: 1,
                max_depth: 8,
            },
            PackageSourceClosureLimitKind::DependencyRequests,
        ),
        (
            PackageSourceClosureLimits {
                max_packages: 8,
                max_dependency_requests: 8,
                max_depth: 1,
            },
            PackageSourceClosureLimitKind::Depth,
        ),
    ] {
        let error = resolve_package_source_closure_with_limits(
            git_root_request(&root),
            root.clone(),
            limits,
            fake_adapter(packages.clone()),
        )
        .expect_err("closure ceiling must reject");
        assert!(matches!(
            error,
            PackageSourceClosureResolutionError::LimitExceeded { kind, .. }
                if kind == expected_kind
        ));
    }
}

#[test]
fn rejects_an_application_selected_through_a_dependency_edge() {
    let dependency = custody_with_role(
        "tool",
        "tool",
        2,
        "/snapshots/tool",
        crate::declarations::BuildDeclarationKind::Application,
        vec![],
    );
    let root = custody("root", "root", 1, "/snapshots/root", vec![request("tool")]);

    let error = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([("tool", dependency)])),
    )
    .expect_err("applications are not importable dependencies");

    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::InvalidDependencyRole {
            dependency_index: 0,
            role: crate::declarations::BuildDeclarationKind::Application,
            ..
        }
    ));
}
