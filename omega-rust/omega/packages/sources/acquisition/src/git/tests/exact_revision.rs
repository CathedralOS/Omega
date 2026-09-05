//! Persisted exact revisions never refresh or acquire an operation-local pin.

use super::*;
use crate::observations::resolved::ResolvedGitSource;
mod failures;
mod fixtures;
mod retry;
use fixtures::Fixture;

#[test]
fn persisted_sha256_objects_fetch_cold_without_discovering_the_authored_selector() {
    let mut fixture = Fixture::with_format("exact-revision-sha256", Some("sha256"));
    assert_eq!(fixture.commit.algorithm(), GitObjectIdAlgorithm::Sha256);
    assert_eq!(fixture.tree.algorithm(), GitObjectIdAlgorithm::Sha256);
    fixture.advance();
    // A missing authored selector cannot provide object-format discovery.
    // Exact persisted IDs still select the earlier reachable commit and tree.
    fixture.request = local_git_request(&fixture.repository, "refs/heads/absent-selector");
    let original = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch)
        .unwrap();
    assert_eq!(original.commit(), fixture.commit.to_hex());
    assert_eq!(original.tree(), fixture.tree.to_hex());
    assert_eq!(original.requested_revision(), "refs/heads/absent-selector");
    assert_eq!(
        std::fs::read(original.snapshot_root().join("main.omg")).unwrap(),
        b"machine Main::main() {}\n"
    );
    fixture.disconnect();
    let reused = fixture
        .resolve(GitExactRevisionAcquisition::Offline)
        .unwrap();
    assert_eq!(reused.commit(), original.commit());
    assert_eq!(reused.tree(), original.tree());
    assert_eq!(reused.content_identity(), original.content_identity());
}

#[test]
fn persisted_objects_fetch_cold_and_ignore_branch_and_fetch_head_movement() {
    let mut fixture = Fixture::new("exact-revision-cold-movement");
    let (advanced, _) = fixture.advance();
    let original = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch)
        .unwrap();
    fixture.assert_original(&original);
    let refreshed = resolve_git_source_in_lane(
        &fixture.request,
        fixture.storage.git_sources(),
        LocalSourceLimits::default(),
    )
    .unwrap();
    assert_eq!(refreshed.commit(), advanced.to_hex());
    assert_ne!(refreshed.tree(), original.tree());
    // The ordinary selector refresh also moves retained FETCH_HEAD. Exact
    // recovery must use the supplied object IDs, not either mutable name.
    fixture.disconnect();
    for mode in [
        GitExactRevisionAcquisition::Offline,
        GitExactRevisionAcquisition::AllowFetch,
    ] {
        let recovered = fixture
            .resolve(mode)
            .expect("warm exact objects need no transport");
        fixture.assert_original(&recovered);
        assert_eq!(recovered.content_identity(), original.content_identity());
        assert_eq!(recovered.lineage(), original.lineage());
    }
}

#[test]
fn persisted_objects_support_explicit_primary_git_without_transport() {
    let mut fixture = Fixture::new("exact-revision-explicit-primary");
    let first = resolve_git_source_at_revision_in_lane_with_primary_git(
        fixture.storage.git_sources().primary_git().unwrap(),
        &fixture.request,
        &fixture.commit,
        &fixture.tree,
        GitExactRevisionAcquisition::AllowFetch,
        fixture.storage.git_sources(),
        LocalSourceLimits::default(),
    )
    .unwrap();
    fixture.assert_original(&first);
    fixture.disconnect();
    let recovered = resolve_git_source_at_revision_in_lane_with_primary_git(
        fixture.storage.git_sources().primary_git().unwrap(),
        &fixture.request,
        &fixture.commit,
        &fixture.tree,
        GitExactRevisionAcquisition::Offline,
        fixture.storage.git_sources(),
        LocalSourceLimits::default(),
    )
    .unwrap();
    fixture.assert_original(&recovered);
    assert_eq!(first.content_identity(), recovered.content_identity());
}

#[test]
fn offline_object_misses_preserve_healthy_cache_and_never_refresh_selector() {
    let mut fixture = Fixture::new("exact-revision-offline-misses");
    let error = fixture
        .resolve(GitExactRevisionAcquisition::Offline)
        .unwrap_err();
    assert!(
        matches!(
            error,
            SourceResolveError::GitExactRevisionUnavailable { .. }
        ),
        "{error:?}"
    );
    assert!(
        !fixture.entry().exists(),
        "cold offline failure must not create a cache"
    );
    fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch)
        .unwrap();
    let metadata = std::fs::read(fixture.entry().join(GIT_CACHE_METADATA)).unwrap();
    let (unfetched_commit, unfetched_tree) = fixture.advance();
    fixture.disconnect();
    let error = fixture
        .resolve_objects(
            &unfetched_commit,
            &unfetched_tree,
            GitExactRevisionAcquisition::Offline,
        )
        .unwrap_err();
    assert!(
        matches!(
            error,
            SourceResolveError::GitExactRevisionUnavailable { .. }
        ),
        "{error:?}"
    );
    assert_eq!(
        std::fs::read(fixture.entry().join(GIT_CACHE_METADATA)).unwrap(),
        metadata
    );
    fixture.assert_original(
        &fixture
            .resolve(GitExactRevisionAcquisition::Offline)
            .unwrap(),
    );
}

#[test]
fn warm_exact_recovery_enforces_current_source_limits() {
    let mut fixture = Fixture::new("exact-revision-current-limits");
    fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch)
        .unwrap();
    fixture.disconnect();
    let error = resolve_git_source_at_revision_in_lane(
        &fixture.request,
        &fixture.commit,
        &fixture.tree,
        GitExactRevisionAcquisition::Offline,
        fixture.storage.git_sources(),
        LocalSourceLimits {
            max_bytes: 0,
            ..LocalSourceLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error, SourceResolveError::TooManyBytes { limit: 0 });
}
