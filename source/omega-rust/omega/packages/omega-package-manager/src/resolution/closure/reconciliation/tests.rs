use super::super::validation::PackageClosureValidationError;
use super::*;
use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::resolution::{
    PackageSourceCustody, ResolvedPackageSource, resolve_workspace_member_package_source,
};
use omega_package_source::{
    AliasName, GitCommitId, GitSourceRequest, GitTreeId, ImmutableSourceResolution,
    LocalSourceLimits, PackageKey, PackageName, SourceContentDigest, SourceLineage,
    WorkspaceMemberPath,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn key(name: &str, repository: &str) -> PackageKey {
    PackageKey::new(
        PackageName::parse(name).expect("valid package name"),
        SourceLineage::git(&format!("https://github.com/CathedralOS/{repository}.git"))
            .expect("valid Git lineage"),
    )
}

fn resolution(marker: u8) -> ImmutableSourceResolution {
    let commit_digit = char::from_digit(u32::from(marker % 10), 16).unwrap();
    let tree_digit = char::from_digit(u32::from((marker + 1) % 10), 16).unwrap();
    ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&commit_digit.to_string().repeat(40)).unwrap(),
        GitTreeId::parse_hex(&tree_digit.to_string().repeat(40)).unwrap(),
        SourceContentDigest::derive(&[marker]),
    )
    .unwrap()
}

fn request(location: &str) -> DependencySourceRequest {
    DependencySourceRequest::Path {
        explicit_alias: None,
        location: location.to_owned(),
    }
}

fn request_as(alias: &str, location: &str) -> DependencySourceRequest {
    DependencySourceRequest::Path {
        explicit_alias: Some(AliasName::parse(alias).expect("valid alias")),
        location: location.to_owned(),
    }
}

fn request_location(request: &DependencySourceRequest) -> &str {
    match request {
        DependencySourceRequest::Path { location, .. } => location,
        DependencySourceRequest::Git { repository, .. } => repository,
    }
}

fn custody(
    name: &str,
    repository: &str,
    marker: u8,
    snapshot_root: &str,
    dependency_requests: Vec<DependencySourceRequest>,
) -> PackageSourceCustody {
    PackageSourceCustody::from_resolved_parts(
        key(name, repository),
        resolution(marker),
        PathBuf::from(snapshot_root),
        LocalSourceLimits::default(),
        dependency_requests,
    )
}

fn git_root_request(root: &PackageSourceCustody) -> PackageRootSourceRequest {
    PackageRootSourceRequest::Git(
        GitSourceRequest::new(
            format!(
                "https://github.com/CathedralOS/{}.git",
                root.key().name().as_str()
            ),
            Some("HEAD".to_owned()),
        )
        .expect("synthetic root request"),
    )
}

fn fake_adapter(
    packages: BTreeMap<&'static str, PackageSourceCustody>,
) -> impl FnMut(
    &PackageSourceCustody,
    &DependencySourceRequest,
) -> Result<PackageSourceCustody, &'static str> {
    move |_, request| {
        packages
            .get(request_location(request))
            .cloned()
            .ok_or("unknown fake source")
    }
}

fn package_fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../tests/fixtures/packages")
}

fn temp_cache() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-closure-{}-{stamp}",
        std::process::id()
    ))
}

fn workspace_member_request(
    requester: &PackageSourceCustody,
    location: &str,
) -> Result<WorkspaceMemberPath, String> {
    let SourceLineage::Workspace(lineage) = requester.key().source_lineage() else {
        return Err("path requester is not a workspace member".to_owned());
    };
    let mut normalized = PathBuf::from(lineage.member_path().as_str());
    for component in Path::new(location).components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            _ => return Err("path request escapes the fixture workspace".to_owned()),
        }
    }
    WorkspaceMemberPath::parse(
        normalized
            .to_str()
            .ok_or_else(|| "fixture member path is not UTF-8".to_owned())?,
    )
    .map_err(|error| error.to_string())
}

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
        WorkspaceMemberPath::parse("graph-workbench").expect("root member path"),
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
            member_path: WorkspaceMemberPath::parse("graph-workbench").expect("root member path"),
            requested_workspace_root: fixtures.clone(),
        },
        root,
        |requester, request| {
            let DependencySourceRequest::Path { location, .. } = request else {
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
        PackageSourceClosureResolutionError::InvalidClosure { ref errors }
            if errors.iter().any(|error| matches!(
                error,
                PackageClosureValidationError::DuplicateAlias { alias, .. }
                    if alias.as_str() == "math"
            ))
    ));
}

#[test]
fn conflicting_resolution_reports_every_requesting_path() {
    let shared_first = custody("shared", "shared", 4, "/snapshots/shared-first", vec![]);
    let shared_conflicting = custody(
        "shared",
        "shared",
        5,
        "/snapshots/shared-conflicting",
        vec![],
    );
    let left = custody(
        "left",
        "left",
        2,
        "/snapshots/left",
        vec![request("shared-first")],
    );
    let right = custody(
        "right",
        "right",
        3,
        "/snapshots/right",
        vec![request("shared-first-again")],
    );
    let conflicting_branch = custody(
        "conflicting-branch",
        "conflicting-branch",
        6,
        "/snapshots/conflicting-branch",
        vec![request("shared-conflicting")],
    );
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![
            request("left"),
            request("right"),
            request("conflicting-branch"),
        ],
    );

    let error = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([
            ("left", left),
            ("right", right),
            ("conflicting-branch", conflicting_branch),
            ("shared-first", shared_first),
            (
                "shared-first-again",
                custody("shared", "shared", 4, "/snapshots/shared-first", vec![]),
            ),
            ("shared-conflicting", shared_conflicting),
        ])),
    )
    .expect_err("same key at conflicting resolutions rejects");
    let conflicts = error.conflicts().expect("custody conflict details");

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].key().name().as_str(), "shared");
    assert_eq!(conflicts[0].candidates().len(), 2);
    let paths: Vec<_> = conflicts[0]
        .candidates()
        .iter()
        .flat_map(PackageSourceClosureConflictCandidate::requesting_paths)
        .collect();
    assert_eq!(
        conflicts[0].candidates()[0].requesting_paths().len(),
        2,
        "exact duplicate custody retains both requesting paths"
    );
    assert_eq!(paths.len(), 3);
    assert!(paths.iter().all(|path| path.steps().len() == 2));
    let first_hops: BTreeSet<_> = paths
        .iter()
        .map(|path| path.steps()[0].target().name().as_str())
        .collect();
    assert_eq!(
        first_hops,
        BTreeSet::from(["conflicting-branch", "left", "right"])
    );
}

#[test]
fn same_key_and_resolution_with_different_custody_root_rejects() {
    let first = custody("shared", "shared", 2, "/snapshots/first", vec![]);
    let mut second = first.clone();
    second.snapshot_root = PathBuf::from("/snapshots/second");
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("first"), request("second")],
    );

    let error = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([("first", first), ("second", second)])),
    )
    .expect_err("custody root drift rejects");
    let conflicts = error.conflicts().expect("custody conflict details");

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].candidates().len(), 2);
    assert_eq!(
        conflicts[0]
            .candidates()
            .iter()
            .map(|candidate| candidate.custody().resolution())
            .collect::<BTreeSet<_>>()
            .len(),
        1,
        "the differing custody roots share one immutable resolution"
    );
}

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
fn returns_adapter_error_with_exact_request_context() {
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request("missing")],
    );

    let error = resolve_package_source_closure(git_root_request(&root), root, |_, _| {
        Err::<PackageSourceCustody, _>("network unavailable")
    })
    .expect_err("adapter failure returns");

    assert!(matches!(
        error,
        PackageSourceClosureResolutionError::Adapter {
            requester,
            dependency_index: 0,
            request: DependencySourceRequest::Path { location, .. },
            error: "network unavailable",
        } if requester.name().as_str() == "application" && location == "missing"
    ));
}
