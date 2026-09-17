use super::super::{
    DependencyRequestPath, DependencyRequestPathStep, DependencyRequestPaths,
    PackageRootSourceRequest, ResolvedPackageSourceClosure, resolve_package_source_closure,
};
use super::support::*;
use crate::resolution::source::{
    ResolvedPackageSource, resolve_workspace_member_package_source_from_hardened_base,
};
use package_source::{LocalSourceLimits, SourceLineage, SourceRelativePath};
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
    let root = resolve_workspace_member_package_source_from_hardened_base(
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
            resolve_workspace_member_package_source_from_hardened_base(
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

    assert_eq!(closure.graph().packages().len(), 4);
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

#[test]
fn batch_explanation_paths_traverse_edges_once_and_preserve_breadth_first_choices() {
    let shared = custody("shared", "shared", 2, "/snapshots/shared", vec![]);
    let shortcut = custody("shortcut", "shortcut", 3, "/snapshots/shortcut", vec![]);
    let mut packages = BTreeMap::from([
        ("shared".to_owned(), shared.clone()),
        ("shortcut".to_owned(), shortcut.clone()),
    ]);
    let mut requests = Vec::new();
    // Deliberately oppose package-key order: authored order must decide ties.
    for ordinal in (0..32).rev() {
        let name = format!("branch-{ordinal:02}");
        requests.push(request(&name));
        packages.insert(
            name.clone(),
            custody(
                &name,
                &name,
                4,
                &format!("/snapshots/{name}"),
                vec![request("shared"), request("shortcut")],
            ),
        );
    }
    requests.push(request("shortcut"));
    let root = custody("root", "root", 1, "/snapshots/root", requests);
    let closure = resolve_package_source_closure(git_root_request(&root), root, |_, request| {
        packages
            .get(request_location(request))
            .cloned()
            .ok_or("unknown source")
    })
    .expect("wide diamond closure");
    let paths = closure
        .dependency_paths()
        .expect("validated graph has its root");
    assert_eq!(paths.traversed_edges, 97);
    let shallow = DependencyRequestPaths::new(&closure, Some(&key("branch-31", "branch-31")))
        .expect("first authored edge");
    assert_eq!(
        shallow.traversed_edges, 1,
        "one-off queries stop on discovery"
    );
    let shared_path = paths.path(shared.key()).unwrap();
    assert_eq!(shared_path.steps().len(), 2);
    assert_eq!(shared_path.steps()[0].alias().as_str(), "branch_31");
    assert_eq!(paths.path(shortcut.key()).unwrap().steps().len(), 1);
    assert!(
        paths
            .path(closure.graph().root())
            .unwrap()
            .steps()
            .is_empty()
    );
    assert!(paths.path(&key("absent", "absent")).is_none());
    for _ in 0..3 {
        for package in closure.graph().packages().iter().rev() {
            let expected = reference_breadth_first_path(&closure, package.source().key());
            assert_eq!(paths.path(package.source().key()), expected);
        }
    }
    assert_eq!(
        paths.traversed_edges, 97,
        "queries never walk outgoing edges again"
    );
}

/// Simple per-query oracle: queue complete paths, unlike the production tree.
fn reference_breadth_first_path(
    closure: &ResolvedPackageSourceClosure,
    target: &crate::declarations::PackageKey,
) -> Option<DependencyRequestPath> {
    use std::collections::VecDeque;
    let root = closure.graph().root();
    let mut pending = VecDeque::from([(root.clone(), Vec::new())]);
    let mut seen = BTreeSet::from([root.clone()]);
    while let Some((requester, steps)) = pending.pop_front() {
        if &requester == target {
            return Some(DependencyRequestPath {
                root: root.clone(),
                steps,
            });
        }
        for dependency in closure.graph().package(&requester)?.dependencies() {
            if !seen.insert(dependency.target().clone()) {
                continue;
            }
            let mut next_steps = steps.clone();
            next_steps.push(DependencyRequestPathStep {
                requester: requester.clone(),
                purpose: dependency.purpose(),
                dependency_index: dependency.dependency_index(),
                alias: dependency.alias().clone(),
                target: dependency.target().clone(),
            });
            pending.push_back((dependency.target().clone(), next_steps));
        }
    }
    None
}
