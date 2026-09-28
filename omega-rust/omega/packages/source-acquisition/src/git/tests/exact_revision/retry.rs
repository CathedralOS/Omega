use super::super::{GIT_CACHE_REPOSITORY, PathBuf};
use super::{
    Fixture, GIT_CACHE_METADATA, GitExactRevisionAcquisition, LocalSourceLimits, ResolvedGitSource,
    SourceResolveError, local_git_request, resolve_git_source_in_lane,
};
fn ordinary(fixture: &Fixture) -> Result<ResolvedGitSource, SourceResolveError> {
    resolve_git_source_in_lane(
        fixture.storage.primary_git().unwrap(),
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
