use super::*;

#[test]
fn exact_tree_and_algorithm_mismatches_reject_without_selector_fallback() {
    let fixture = Fixture::new("exact-revision-wrong-objects");
    let (_, other_tree) = fixture.advance();
    let error = fixture
        .resolve_objects(
            &fixture.commit,
            &other_tree,
            GitExactRevisionAcquisition::AllowFetch,
        )
        .unwrap_err();
    assert!(
        matches!(error, SourceResolveError::GitObjectInvalid { .. }),
        "{error:?}"
    );
    let sha256 = GitCommitId::parse_hex(&"12".repeat(32)).unwrap();
    let error = fixture
        .resolve_objects(
            &sha256,
            &fixture.tree,
            GitExactRevisionAcquisition::AllowFetch,
        )
        .unwrap_err();
    assert!(
        matches!(error, SourceResolveError::GitObjectInvalid { .. }),
        "{error:?}"
    );
}

#[test]
fn authored_exact_commit_must_equal_persisted_commit_before_acquisition() {
    let mut fixture = Fixture::new("exact-revision-authored-mismatch");
    let (advanced, _) = fixture.advance();
    let request = local_git_request(&fixture.repository, &advanced.to_hex());
    let request_entry = fixture.request_entry(&request);
    fixture.disconnect();
    let error = resolve_git_source_at_revision_in_lane(
        &request,
        &fixture.commit,
        &fixture.tree,
        GitExactRevisionAcquisition::AllowFetch,
        fixture.storage.git_sources(),
        LocalSourceLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, SourceResolveError::GitObjectInvalid { .. }),
        "{error:?}"
    );
    assert!(
        !request_entry.exists(),
        "rejected exact request must not create its cache entry"
    );
}

#[test]
fn unavailable_persisted_commit_never_falls_back_to_available_head() {
    let fixture = Fixture::new("exact-revision-unavailable");
    let missing = GitCommitId::parse_hex(&"ab".repeat(20)).unwrap();
    assert_ne!(missing, fixture.commit);
    fixture
        .resolve_objects(
            &missing,
            &fixture.tree,
            GitExactRevisionAcquisition::AllowFetch,
        )
        .expect_err("available HEAD cannot replace the unavailable persisted commit");
}

#[test]
fn persisted_objects_reject_tampered_cache_or_snapshot_without_repair_fetch() {
    for snapshot in [false, true] {
        let mut fixture = Fixture::new("exact-revision-tamper");
        let first = fixture
            .resolve(GitExactRevisionAcquisition::AllowFetch)
            .unwrap();
        let altered = if snapshot {
            first.snapshot_root().join("main.omg")
        } else {
            fixture.entry().join(GIT_CACHE_METADATA)
        };
        let mut permissions = std::fs::metadata(&altered).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(permissions.mode() | 0o200);
        }
        #[cfg(not(unix))]
        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "Windows read-only attributes do not change Unix permission bits"
        )]
        permissions.set_readonly(false);
        std::fs::set_permissions(&altered, permissions).unwrap();
        std::fs::write(&altered, b"changed retained source\n").unwrap();
        fixture.disconnect();
        let error = fixture
            .resolve(GitExactRevisionAcquisition::AllowFetch)
            .unwrap_err();
        assert!(
            matches!(error, SourceResolveError::GitCacheInvalid { .. }),
            "{error:?}"
        );
    }
}
