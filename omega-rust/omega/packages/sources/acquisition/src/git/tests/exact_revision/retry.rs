use super::*;

fn ordinary(fixture: &Fixture) -> Result<ResolvedGitSource, SourceResolveError> {
    resolve_git_source_in_lane(
        &fixture.request,
        fixture.storage.git_sources(),
        LocalSourceLimits::default(),
    )
}

fn reconnect(fixture: &mut Fixture, original: PathBuf) {
    std::fs::rename(&fixture.repository, &original).unwrap();
    fixture.repository = original;
}

fn assert_transport_failure(error: SourceResolveError) {
    assert!(matches!(error, SourceResolveError::Git { .. }), "{error:?}");
    assert!(!error.to_string().contains("source.identity"), "{error}");
}

#[test]
fn cold_full_id_fetch_failure_retries_without_manual_cache_changes() {
    let mut fixture = Fixture::new("retry-cold-full-id");
    fixture.request = local_git_request(&fixture.repository, &fixture.commit.to_hex());
    let original = fixture.repository.clone();
    fixture.disconnect();
    for _ in 0..2 {
        assert_transport_failure(ordinary(&fixture).unwrap_err());
        assert!(fixture.entry().join(GIT_CACHE_METADATA).is_file());
    }
    reconnect(&mut fixture, original);
    let resolved = ordinary(&fixture).unwrap();
    assert_eq!(resolved.commit(), fixture.commit.to_hex());
    assert_eq!(resolved.tree(), fixture.tree.to_hex());
    fixture.disconnect();
    assert_eq!(ordinary(&fixture).unwrap().commit(), resolved.commit());
}

#[test]
fn cold_selector_discovery_failure_does_not_publish_an_entry() {
    let mut fixture = Fixture::new("retry-cold-selector");
    let original = fixture.repository.clone();
    fixture.disconnect();
    for _ in 0..2 {
        assert_transport_failure(ordinary(&fixture).unwrap_err());
        assert!(!fixture.entry().exists());
    }
    reconnect(&mut fixture, original);
    fixture.assert_original(&ordinary(&fixture).unwrap());
}

#[test]
fn failed_selector_refresh_keeps_old_exact_objects_usable_offline() {
    let mut fixture = Fixture::new("retry-warm-selector");
    let accepted = ordinary(&fixture).unwrap();
    let metadata = std::fs::read(fixture.entry().join(GIT_CACHE_METADATA)).unwrap();
    let (new_commit, _) = fixture.advance();
    let original = fixture.repository.clone();
    fixture.disconnect();
    for _ in 0..2 {
        assert_transport_failure(ordinary(&fixture).unwrap_err());
        assert_eq!(
            std::fs::read(fixture.entry().join(GIT_CACHE_METADATA)).unwrap(),
            metadata
        );
        fixture.assert_original(
            &fixture
                .resolve(GitExactRevisionAcquisition::Offline)
                .unwrap(),
        );
        assert_eq!(
            std::fs::read(accepted.snapshot_root().join("main.omg")).unwrap(),
            b"machine Main::main() {}\n"
        );
    }
    reconnect(&mut fixture, original);
    assert_eq!(ordinary(&fixture).unwrap().commit(), new_commit.to_hex());
}

#[test]
fn exact_fetch_failure_retries_the_recorded_commit_not_the_selector() {
    let mut fixture = Fixture::new("retry-recorded-cold");
    fixture.advance();
    fixture.request = local_git_request(&fixture.repository, "refs/heads/nonexistent");
    let original = fixture.repository.clone();
    fixture.disconnect();
    for _ in 0..2 {
        assert_transport_failure(
            fixture
                .resolve(GitExactRevisionAcquisition::AllowFetch)
                .unwrap_err(),
        );
        assert!(fixture.entry().join(GIT_CACHE_METADATA).is_file());
    }
    reconnect(&mut fixture, original);
    let resolved = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch)
        .unwrap();
    assert_eq!(resolved.commit(), fixture.commit.to_hex());
    assert_eq!(resolved.tree(), fixture.tree.to_hex());
    assert_eq!(resolved.requested_revision(), "refs/heads/nonexistent");
}

#[test]
fn incomplete_entry_rebuilds_including_readonly_materialized_snapshots() {
    let fixture = Fixture::new("retry-incomplete-snapshot");
    let initial = ordinary(&fixture).unwrap();
    assert!(initial.snapshot_root().is_dir());
    let (new_commit, _) = fixture.advance();
    // Model a prior interrupted/invalidation path. The existing object graph
    // must not be blessed by manufacturing its missing metadata record.
    std::fs::remove_file(fixture.entry().join(GIT_CACHE_METADATA)).unwrap();
    let resolved = ordinary(&fixture).unwrap();
    assert_eq!(resolved.commit(), new_commit.to_hex());
    assert!(fixture.entry().join(GIT_CACHE_METADATA).is_file());
}

#[test]
fn incomplete_exact_cache_is_untouched_offline_and_rebuilt_only_at_recorded_pin() {
    let mut fixture = Fixture::new("retry-incomplete-recorded");
    fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch)
        .unwrap();
    let config = fixture.entry().join(GIT_CACHE_REPOSITORY).join("config");
    let before = std::fs::read(&config).unwrap();
    std::fs::remove_file(fixture.entry().join(GIT_CACHE_METADATA)).unwrap();
    fixture.advance();
    let original = fixture.repository.clone();
    fixture.disconnect();
    assert!(
        fixture
            .resolve(GitExactRevisionAcquisition::Offline)
            .is_err()
    );
    assert!(!fixture.entry().join(GIT_CACHE_METADATA).exists());
    assert_eq!(std::fs::read(&config).unwrap(), before);
    reconnect(&mut fixture, original);
    fixture.assert_original(
        &fixture
            .resolve(GitExactRevisionAcquisition::AllowFetch)
            .unwrap(),
    );
}

#[test]
fn an_operation_local_pin_cannot_rebuild_its_incomplete_cache() {
    let fixture = Fixture::new("retry-incomplete-local-pin");
    let initial = ordinary(&fixture).unwrap();
    let pin = initial.acquisition_pin();
    let snapshot = initial.snapshot_root().to_path_buf();
    std::fs::remove_file(fixture.entry().join(GIT_CACHE_METADATA)).unwrap();
    let result = resolve_git_source_from_pin_in_lane(
        &fixture.request,
        Some(&pin),
        fixture.storage.git_sources(),
        LocalSourceLimits::default(),
    );
    assert!(result.is_err());
    assert!(!fixture.entry().join(GIT_CACHE_METADATA).exists());
    assert!(snapshot.is_dir());
}

#[test]
fn corrupt_metadata_rejects_before_a_later_fresh_acquisition_rebuilds() {
    let fixture = Fixture::new("retry-corrupt-metadata");
    ordinary(&fixture).unwrap();
    std::fs::write(
        fixture.entry().join(GIT_CACHE_METADATA),
        b"foreign metadata",
    )
    .unwrap();
    assert!(matches!(
        ordinary(&fixture),
        Err(SourceResolveError::GitCacheInvalid { .. })
    ));
    assert!(!fixture.entry().join(GIT_CACHE_METADATA).exists());
    fixture.assert_original(&ordinary(&fixture).unwrap());
}

#[cfg(unix)]
#[test]
fn incomplete_entry_symlink_does_not_remove_its_target() {
    let fixture = Fixture::new("retry-entry-symlink");
    let external = fixture.repository.join("not-cache");
    std::fs::create_dir(&external).unwrap();
    let sentinel = external.join("keep");
    std::fs::write(&sentinel, b"not resolver state").unwrap();
    std::os::unix::fs::symlink(&external, fixture.entry()).unwrap();
    assert!(ordinary(&fixture).is_err());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"not resolver state");
}
