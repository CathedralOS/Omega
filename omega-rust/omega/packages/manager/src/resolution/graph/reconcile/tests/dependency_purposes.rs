use super::super::{
    PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    resolve_package_source_closure, resolve_package_source_closure_with_indexed_limits,
};
use super::support::*;
use crate::declarations::dependencies::read::DependencyPurpose;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[test]
fn traverses_product_and_build_rows_with_purpose_and_scope_ordinals() {
    let product = custody(
        "product-lib",
        "product-lib",
        2,
        "/snapshots/product",
        vec![],
    );
    let host = custody("host-tool", "host-tool", 3, "/snapshots/host", vec![]);
    let root = custody_with_scopes(
        "application",
        "application",
        1,
        "/snapshots/application",
        crate::declarations::BuildDeclarationKind::Package,
        vec![request("product")],
        vec![request_as("host_tool", "host")],
    );
    let calls = RefCell::new(Vec::new());
    let packages = BTreeMap::from([("product", product.clone()), ("host", host.clone())]);

    let closure = resolve_package_source_closure_with_indexed_limits(
        git_root_request(&root),
        root,
        PackageSourceClosureLimits::default(),
        |requester, purpose, index, request| {
            calls.borrow_mut().push((
                requester.key().name().as_str().to_owned(),
                purpose,
                index,
                request_location(request).to_owned(),
            ));
            packages
                .get(request_location(request))
                .cloned()
                .ok_or("unknown fake source")
        },
    )
    .expect("dual-scope closure resolves");

    assert_eq!(
        *calls.borrow(),
        [
            (
                "application".to_owned(),
                DependencyPurpose::Product,
                0,
                "product".to_owned()
            ),
            (
                "application".to_owned(),
                DependencyPurpose::Build,
                0,
                "host".to_owned()
            ),
        ]
    );
    assert_eq!(closure.graph().packages().len(), 3);
    let dependencies = closure
        .graph()
        .package(closure.graph().root())
        .expect("root node")
        .dependencies();
    assert_eq!(dependencies.len(), 2);
    assert_eq!(dependencies[0].purpose(), DependencyPurpose::Product);
    assert_eq!(dependencies[0].dependency_index(), 0);
    assert_eq!(dependencies[0].target(), product.key());
    assert_eq!(dependencies[1].purpose(), DependencyPurpose::Build);
    assert_eq!(dependencies[1].dependency_index(), 0);
    assert_eq!(dependencies[1].alias().as_str(), "host_tool");
    assert_eq!(dependencies[1].target(), host.key());
    let path = closure
        .dependency_path(host.key())
        .expect("host selection retains its requesting path");
    assert_eq!(path.steps().len(), 1);
    assert_eq!(path.steps()[0].purpose(), DependencyPurpose::Build);
}

#[test]
fn the_same_alias_is_distinct_across_product_and_build_scopes() {
    let product = custody(
        "product-lib",
        "product-lib",
        2,
        "/snapshots/product",
        vec![],
    );
    let host = custody("host-tool", "host-tool", 3, "/snapshots/host", vec![]);
    let root = custody_with_scopes(
        "application",
        "application",
        1,
        "/snapshots/application",
        crate::declarations::BuildDeclarationKind::Package,
        vec![request_as("shared", "product")],
        vec![request_as("shared", "host")],
    );

    let closure = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([("product", product), ("host", host)])),
    )
    .expect("an alias shared across scopes is not a conflict");

    let purposes = closure
        .graph()
        .package(closure.graph().root())
        .expect("root node")
        .dependencies()
        .iter()
        .map(|dependency| dependency.purpose())
        .collect::<Vec<_>>();
    assert_eq!(
        purposes,
        [DependencyPurpose::Product, DependencyPurpose::Build]
    );
}

#[test]
fn rejects_a_duplicate_alias_inside_the_build_scope() {
    let first = custody("first", "first", 2, "/snapshots/first", vec![]);
    let second = custody("second", "second", 3, "/snapshots/second", vec![]);
    let root = custody_with_scopes(
        "application",
        "application",
        1,
        "/snapshots/application",
        crate::declarations::BuildDeclarationKind::Package,
        Vec::new(),
        vec![request_as("tool", "first"), request_as("tool", "second")],
    );

    let error = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([("first", first), ("second", second)])),
    )
    .expect_err("duplicate build alias rejects");

    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::InvalidAliases {
            purpose: DependencyPurpose::Build,
            ..
        }
    ));
}

#[test]
fn nested_build_rows_retain_their_own_requester_and_purpose() {
    let product = custody_with_scopes(
        "product-lib",
        "product-lib",
        2,
        "/snapshots/product",
        crate::declarations::BuildDeclarationKind::Package,
        Vec::new(),
        vec![request("nested-host")],
    );
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("product")],
    );

    let nested = custody("nested-host", "nested-host", 3, "/snapshots/nested", vec![]);
    let closure = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([
            ("product", product.clone()),
            ("nested-host", nested.clone()),
        ])),
    )
    .expect("source acquisition retains a dependency's own build inputs");

    let dependencies = closure
        .graph()
        .package(product.key())
        .unwrap()
        .dependencies();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].purpose(), DependencyPurpose::Build);
    assert_eq!(dependencies[0].dependency_index(), 0);
    assert_eq!(dependencies[0].target(), nested.key());
    let path = closure.dependency_path(nested.key()).unwrap();
    assert_eq!(path.steps().len(), 2);
    assert_eq!(path.steps()[0].purpose(), DependencyPurpose::Product);
    assert_eq!(path.steps()[1].purpose(), DependencyPurpose::Build);
}

#[test]
fn rejects_a_cycle_spanning_product_and_nested_build_dependencies() {
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("library")],
    );
    let library = custody_with_scopes(
        "library",
        "library",
        2,
        "/snapshots/library",
        crate::declarations::BuildDeclarationKind::Package,
        Vec::new(),
        vec![request("root-again")],
    );
    let error = resolve_package_source_closure(
        git_root_request(&root),
        root.clone(),
        fake_adapter(BTreeMap::from([("library", library), ("root-again", root)])),
    )
    .expect_err("purpose boundaries do not hide an acquisition cycle");

    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::InvalidClosure { ref errors }
            if errors.iter().any(|error| matches!(
                error,
                crate::resolution::graph::PackageClosureValidationError::DependencyCycle { .. }
            ))
    ));
}

#[test]
fn an_unresolvable_build_request_reports_its_purpose_and_ordinal() {
    let root = custody_with_scopes(
        "application",
        "application",
        1,
        "/snapshots/application",
        crate::declarations::BuildDeclarationKind::Package,
        vec![request("product")],
        vec![request("missing-host")],
    );
    let product = custody(
        "product-lib",
        "product-lib",
        2,
        "/snapshots/product",
        vec![],
    );

    let error = resolve_package_source_closure_with_indexed_limits(
        git_root_request(&root),
        root,
        PackageSourceClosureLimits::default(),
        |_, _, _, request| {
            BTreeMap::from([("product", product.clone())])
                .get(request_location(request))
                .cloned()
                .ok_or("unknown fake source")
        },
    )
    .expect_err("the missing build selection fails with its exact coordinate");

    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::Adapter {
            purpose: DependencyPurpose::Build,
            dependency_index: 0,
            ..
        }
    ));
}
