use super::super::*;
use super::support::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
fn same_semantic_custody_at_different_cache_roots_deduplicates() {
    let first = custody("shared", "shared", 2, "/snapshots/first", vec![]);
    let mut second = first.clone();
    second.snapshot_root = PathBuf::from("/snapshots/second");
    let root = custody(
        "application",
        "application",
        1,
        "/snapshots/application",
        vec![request_as("first", "first"), request_as("second", "second")],
    );

    let closure = resolve_package_source_closure(
        git_root_request(&root),
        root,
        fake_adapter(BTreeMap::from([("first", first), ("second", second)])),
    )
    .expect("cache relocation does not change package semantics");

    let shared = closure
        .custodies()
        .iter()
        .find(|custody| custody.key().name().as_str() == "shared")
        .expect("deduplicated shared custody");
    assert_eq!(shared.snapshot_root(), Path::new("/snapshots/first"));
}
