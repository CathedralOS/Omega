use super::super::*;
use super::support::*;
use crate::manifest::dependencies::read::ActiveDependencyAliasError;
use std::collections::BTreeMap;

#[test]
fn derives_default_alias_and_honors_explicit_alias() {
    let ordinary = custody(
        "arithmetic-kernels",
        "arithmetic-kernels",
        2,
        "/snapshots/arithmetic-kernels",
        vec![],
    );
    let renamed = custody(
        "exact-math",
        "exact-math",
        3,
        "/snapshots/exact-math",
        vec![],
    );
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("ordinary"), request_as("integer_math", "renamed")],
    );
    let root_key = root.key().clone();

    let closure = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([
            ("ordinary", ordinary),
            ("renamed", renamed),
        ])),
    )
    .expect("aliases resolve");
    let aliases: Vec<_> = closure
        .graph()
        .package(&root_key)
        .expect("root node")
        .dependencies()
        .iter()
        .map(|dependency| dependency.alias().as_str())
        .collect();

    assert_eq!(aliases, ["arithmetic_kernels", "integer_math"]);
}

#[test]
fn rejects_duplicate_requester_local_alias_after_resolution() {
    let first = custody("first", "first", 2, "/snapshots/first", vec![]);
    let second = custody("second", "second", 3, "/snapshots/second", vec![]);
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request_as("math", "first"), request_as("math", "second")],
    );

    let error = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([("first", first), ("second", second)])),
    )
    .expect_err("duplicate alias rejects");

    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::InvalidActiveAliases {
            error: ActiveDependencyAliasError::DuplicateAlias { ref alias, .. },
            ..
        } if alias.as_str() == "math"
    ));
}
