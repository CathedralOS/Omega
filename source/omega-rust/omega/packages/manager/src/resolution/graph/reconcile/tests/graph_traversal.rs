use super::super::*;
use super::support::*;
use crate::resolution::source::{ResolvedPackageSource, resolve_workspace_member_package_source};
use omega_package_source::{LocalSourceLimits, SourceLineage, SourceRelativePath};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[test]
fn resolves_diamond_once_while_visiting_every_authored_request() {
    let shared = custody("shared-math", "shared-math", 4, "/snapshots/shared", vec![]);
    let left = custody(
        "left-math",
        "left-math",
        2,
        "/snapshots/left",
        vec![request("shared-from-left")],
    );
    let right = custody(
        "right-math",
        "right-math",
        3,
        "/snapshots/right",
        vec![request("shared-from-right")],
    );
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("left"), request("right")],
    );
    let calls = RefCell::new(Vec::new());
    let packages = BTreeMap::from([
        ("left", left),
        ("right", right),
        ("shared-from-left", shared.clone()),
        ("shared-from-right", shared.clone()),
    ]);

    let closure =
        resolve_package_source_closure(git_root_request(&root), root, |requester, request| {
            calls.borrow_mut().push((
                requester.key().name().as_str().to_owned(),
                request_location(request).to_owned(),
            ));
            packages
                .get(request_location(request))
                .cloned()
                .ok_or("unknown fake source")
        })
        .expect("diamond closure resolves");

    assert_eq!(closure.graph().packages().len(), 4);
    assert_eq!(closure.custodies().len(), 4);
    assert_eq!(
        closure.source_root(shared.key()),
        Some(Path::new("/snapshots/shared"))
    );
    assert_eq!(calls.borrow().len(), 4, "every authored row is resolved");
    assert_eq!(
        closure
            .graph()
            .package(shared.key())
            .expect("shared node")
            .dependencies(),
        []
    );
    let path = closure
        .dependency_path(shared.key())
        .expect("shared package has one bounded explanation path");
    assert_eq!(path.root().name().as_str(), "application");
    assert_eq!(path.steps().len(), 2);
    assert_eq!(path.steps()[0].alias().as_str(), "left_math");
    assert_eq!(path.steps()[1].alias().as_str(), "shared_math");
    assert!(
        closure
            .dependency_path(closure.graph().root())
            .unwrap()
            .steps()
            .is_empty()
    );
    assert!(closure.dependency_path(&key("absent", "absent")).is_none());

    let requests = closure.source_requests();
    let root_binding = requests.root();
    let PackageRootSourceRequest::Git(root_request) = root_binding.request() else {
        panic!("synthetic root retains its Git request")
    };
    assert_eq!(
        root_request.requested_locator(),
        "https://github.com/CathedralOS/application.git"
    );
    assert_eq!(root_request.requested_revision(), "HEAD");
    assert_eq!(root_binding.selected().key(), closure.graph().root());

    let dependency_bindings = requests.dependencies().collect::<Vec<_>>();
    assert_eq!(dependency_bindings.len(), 4);
    let shared_bindings = dependency_bindings
        .iter()
        .filter(|binding| binding.selected().key() == shared.key())
        .collect::<Vec<_>>();
    assert_eq!(shared_bindings.len(), 2);
    assert_eq!(
        shared_bindings
            .iter()
            .map(|binding| request_location(binding.request()))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["shared-from-left", "shared-from-right"])
    );
    assert!(
        shared_bindings
            .iter()
            .all(|binding| binding.alias().as_str() == "shared_math")
    );
}

#[test]
fn resolves_the_authored_local_graph_fixture() {
    let fixtures = package_fixtures_root();
    let cache = temp_cache();
    let workspace_source =
        SourceLineage::git("https://github.com/CathedralOS/package-fixtures.git")
            .expect("fixture workspace lineage");
    let root = resolve_workspace_member_package_source(
        &workspace_source,
        SourceRelativePath::parse("graph-workbench").expect("root member path"),
        &fixtures,
        &cache,
        LocalSourceLimits::default(),
    )
    .expect("resolve fixture root")
    .into_custody();
    let root_key = root.key().clone();

    let closure = resolve_package_source_closure(
        PackageRootSourceRequest::WorkspaceMember {
            workspace_root_source: workspace_source.clone(),
            member_path: SourceRelativePath::parse("graph-workbench").expect("root member path"),
            requested_workspace_root: fixtures.clone(),
        },
        root,
        |requester, request| {
            let crate::declarations::dependencies::read::DependencySourceRequest::Path {
                location,
                ..
            } = request
            else {
                return Err("fixture unexpectedly requested a network source".to_owned());
            };
            let member = workspace_member_request(requester, location)?;
            resolve_workspace_member_package_source(
                &workspace_source,
                member,
                &fixtures,
                &cache,
                LocalSourceLimits::default(),
            )
            .map(ResolvedPackageSource::into_custody)
            .map_err(|error| error.to_string())
        },
    )
    .expect("resolve authored fixture closure");

    assert_eq!(closure.graph().packages().len(), 3);
    let aliases = closure
        .graph()
        .package(&root_key)
        .expect("root graph node")
        .dependencies()
        .iter()
        .map(|dependency| dependency.alias().as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        aliases,
        BTreeSet::from(["arithmetic_kernels", "file_journal"])
    );

    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn traverses_complete_transitive_closure_before_returning() {
    let leaf = custody("leaf", "leaf", 4, "/snapshots/leaf", vec![]);
    let third = custody(
        "third",
        "third",
        3,
        "/snapshots/third",
        vec![request("leaf")],
    );
    let second = custody(
        "second",
        "second",
        2,
        "/snapshots/second",
        vec![request("third")],
    );
    let root = custody(
        "root",
        "root",
        1,
        "/snapshots/root",
        vec![request("second")],
    );
    let leaf_key = leaf.key().clone();

    let closure = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([
            ("second", second),
            ("third", third),
            ("leaf", leaf),
        ])),
    )
    .expect("transitive closure resolves");

    assert_eq!(closure.graph().packages().len(), 4);
    assert!(closure.custody(&leaf_key).is_some());
}
