use super::*;
use crate::resolution::graph::CanonicalDependencySourceRequest;

#[test]
fn preserved_repository_pin_applies_to_another_named_member() {
    let fixture = Fixture::workspace("preserved-named-members");
    let storage = fixture.storage("warm");
    let left_request = fixture.named("left");
    let accepted = fixture.subject(&left_request, &storage, false);
    fixture.advance("packages/right/main.omg");
    fixture.disconnect();
    let policy =
        GitDependencyPins::new(&accepted, &[], GitExactRevisionAcquisition::Offline).unwrap();
    let mut cache = GitAcquisitionCache::preserving(policy);

    let left = resolve(&mut cache, &left_request, &storage, false).unwrap();
    let right = resolve(&mut cache, &fixture.named("right"), &storage, false).unwrap();
    assert_eq!(left.resolution(), accepted.root().selected().resolution());
    assert_eq!(right.resolution(), left.resolution());
    assert_ne!(right.selection_evidence(), left.selection_evidence());
    assert_eq!(right.key(), &package_key(&accepted, "right"));
    assert_eq!(
        std::fs::read_to_string(right.snapshot_root().join("main.omg")).unwrap(),
        "machine root() {}\n"
    );
    assert_eq!(cache.acquisition_count(), 1);
}

#[test]
fn selecting_path_only_sibling_refreshes_the_entire_git_source_lineage() {
    let fixture = Fixture::workspace("selected-path-sibling");
    let storage = fixture.storage("warm");
    let left_request = fixture.named("left");
    let accepted = fixture.subject(&left_request, &storage, false);
    let right_key = package_key(&accepted, "right");
    assert_eq!(accepted.packages().len(), 2);
    assert_eq!(accepted.dependency_requests().len(), 1);
    let edge = &accepted.dependency_requests()[0];
    assert_eq!(edge.selected().key(), &right_key);
    assert!(
        matches!(edge.request(), CanonicalDependencySourceRequest::Path { location, .. } if location == "../right")
    );
    assert_eq!(
        right_key.source_lineage(),
        left_request.acquisition().lineage()
    );
    fixture.advance("packages/right/main.omg");
    let moved_commit = test_git_head(&fixture.repository);
    let selection = [right_key.clone()];
    let policy =
        GitDependencyPins::new(&accepted, &selection, GitExactRevisionAcquisition::Offline)
            .unwrap();
    let mut cache = GitAcquisitionCache::preserving(policy);

    // The only selected key was reached by Path, but resolving the left Git
    // request must refresh its shared repository before any sibling is read.
    let left = resolve(&mut cache, &left_request, &storage, false).unwrap();
    assert_eq!(left.source().commit(), moved_commit);
    assert_ne!(left.resolution(), accepted.root().selected().resolution());
    fixture.advance("packages/left/later.omg");
    let right = resolve(&mut cache, &fixture.named("right"), &storage, false).unwrap();
    assert_eq!(right.key(), &right_key);
    assert_eq!(right.resolution(), left.resolution());
    assert_eq!(right.source().commit(), moved_commit);
    assert_eq!(
        std::fs::read_to_string(right.snapshot_root().join("main.omg")).unwrap(),
        "machine advanced() {}\n"
    );
    assert_eq!(cache.acquisition_count(), 1);
}
