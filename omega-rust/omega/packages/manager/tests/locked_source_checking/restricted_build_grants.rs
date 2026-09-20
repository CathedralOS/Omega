//! A confined generator needs no restricted host decision when its retained
//! lock is checked from a fresh captured source and private staging session.
//! Actual restricted-request join controls live beside candidate compilation.

use super::generated::generated_workspace;
use super::{LockedSourceRecoveryOptions, TARGET, Tree, capture_lock, check_locked_sources, fs};
use package_evidence::record::PackagePolicyRowKind;
use package_manager::review::ungranted_restricted_build_requests;

#[test]
fn confined_generator_checks_from_a_lock_without_restricted_host_decisions() {
    let tree = Tree::new();
    let storage = tree.storage("old-cache");
    let closure = generated_workspace(&tree, &storage);
    let (lock, request) = capture_lock(&closure, &tree.path("old-build"));
    assert!(
        lock.target(TARGET)
            .unwrap()
            .occurrences()
            .iter()
            .all(|occurrence| {
                occurrence
                    .acceptance()
                    .rows()
                    .iter()
                    .all(|row| row.kind() != PackagePolicyRowKind::RestrictedBuildRequest)
            })
    );
    fs::remove_dir_all(tree.path("old-build")).unwrap();
    let storage = tree.storage("new-cache");
    let checked = check_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("fresh-build"),
    )
    .expect("locked checking needs no host consent for confined generation");
    let producer = checked
        .reviews()
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-table")
        .expect("the generated producer reviewed");
    assert!(
        producer.restricted_build_requests().is_empty(),
        "captured input and private staging carry no restricted host request"
    );
    assert!(
        ungranted_restricted_build_requests(checked.accepted(), checked.reviews())
            .unwrap()
            .is_empty()
    );
}
