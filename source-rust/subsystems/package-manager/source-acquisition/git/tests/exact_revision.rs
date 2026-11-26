// Exact-revision Git resolution tests verifying that persisted commit and tree
// IDs fetch cold without selector discovery, preserve healthy caches on offline
// misses, and enforce current source limits on warm recovery.

use super::{
    GIT_CACHE_METADATA, GIT_CACHE_REPOSITORY, GitCommitId, GitExactRevisionAcquisition,
    GitObjectIdAlgorithm, GitSourceRequest, GitTreeId, LocalSourceLimits, PathBuf,
    SourceResolveError, SourceResolver, create_git_source_with_format, fixture_storage,
    git_cache_identity, local_git_request, remove_fixture, resolve_git_source_at_revision_in_lane,
    resolve_git_source_in_lane, run_test_git, run_test_git_with_input, temp_root,
};
use crate::observations::resolved::ResolvedGitSource;

#[test]
fn an_authored_commit_supplies_its_authenticated_tree_offline() {
    let mut fixture = Fixture::new("authored-exact-commit-offline");
    fixture.request = local_git_request(&fixture.repository, &fixture.commit.to_hex());
    let recover = |fixture: &Fixture| {
        resolve_git_source_at_revision_in_lane(
            fixture.storage.primary_git().unwrap(),
            &fixture.request,
            &fixture.commit,
            None,
            GitExactRevisionAcquisition::Offline,
            fixture.storage.git_sources(),
            LocalSourceLimits::default(),
        )
    };
    assert!(matches!(
        recover(&fixture),
        Err(SourceResolveError::GitExactRevisionUnavailable { .. })
    ));
    let fetched = resolve_git_source_at_revision_in_lane(
        fixture.storage.primary_git().unwrap(),
        &fixture.request,
        &fixture.commit,
        None,
        GitExactRevisionAcquisition::AllowFetch,
        fixture.storage.git_sources(),
        LocalSourceLimits::default(),
    )
    .unwrap();
    fixture.disconnect();
    let rebuilt = recover(&fixture).unwrap();
    assert_eq!(rebuilt.commit(), fetched.commit());
    assert_eq!(rebuilt.tree(), fetched.tree());
    assert_eq!(
        std::fs::read(rebuilt.snapshot_root().join("main.omg")).unwrap(),
        b"machine Main::main() {}\n"
    );
}

#[test]
fn persisted_sha256_objects_fetch_cold_without_discovering_the_authored_selector() {
    let mut fixture = Fixture::with_format("exact-revision-sha256", Some("sha256"));
    assert_eq!(fixture.commit.algorithm(), GitObjectIdAlgorithm::Sha256);
    assert_eq!(fixture.tree.algorithm(), GitObjectIdAlgorithm::Sha256);
    fixture.advance();
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
        fixture.storage.primary_git().unwrap(),
        &fixture.request,
        &fixture.commit,
        Some(&fixture.tree),
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

// Failure-mode tests for exact-revision Git resolution, verifying that tree/hash
// mismatches and authored-vs-persisted commit conflicts reject before fetch.

#[test]
pub(in crate::git::tests::exact_revision) fn exact_tree_and_algorithm_mismatches_reject_without_selector_fallback()
 {
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
pub(in crate::git::tests::exact_revision) fn authored_exact_commit_must_equal_persisted_commit_before_acquisition()
 {
    let mut fixture = Fixture::new("exact-revision-authored-mismatch");
    let (advanced, _) = fixture.advance();
    let request = local_git_request(&fixture.repository, &advanced.to_hex());
    let request_entry = fixture.request_entry(&request);
    fixture.disconnect();
    let error = resolve_git_source_at_revision_in_lane(
        fixture.storage.primary_git().unwrap(),
        &request,
        &fixture.commit,
        Some(&fixture.tree),
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

// Test fixture harness for exact-revision Git resolution, holding persisted hex
// commit and tree IDs alongside a local Git repository and hardened storage.

struct Fixture {
    pub(super) repository: PathBuf,
    storage_base: PathBuf,
    pub(super) storage: SourceResolver,
    pub(super) request: GitSourceRequest,
    pub(super) commit: GitCommitId,
    pub(super) tree: GitTreeId,
}

impl Fixture {
    pub(super) fn new(name: &str) -> Self {
        Self::with_format(name, None)
    }

    pub(super) fn with_format(name: &str, format: Option<&str>) -> Self {
        let (repository, commit) = create_git_source_with_format(name, format);
        let tree = run_test_git_with_input(&repository, ["rev-parse", "HEAD^{tree}"], b"");
        let request = local_git_request(&repository, "HEAD");
        let storage_base = temp_root(name);
        let storage = fixture_storage(&storage_base, &request);
        Self {
            repository,
            storage_base,
            storage,
            request,
            commit: GitCommitId::parse_hex(&commit).unwrap(),
            tree: GitTreeId::parse_hex(&tree).unwrap(),
        }
    }

    pub(super) fn resolve(
        &self,
        mode: GitExactRevisionAcquisition,
    ) -> Result<ResolvedGitSource, SourceResolveError> {
        self.resolve_objects(&self.commit, &self.tree, mode)
    }

    pub(super) fn resolve_objects(
        &self,
        commit: &GitCommitId,
        tree: &GitTreeId,
        mode: GitExactRevisionAcquisition,
    ) -> Result<ResolvedGitSource, SourceResolveError> {
        resolve_git_source_at_revision_in_lane(
            self.storage.primary_git().unwrap(),
            &self.request,
            commit,
            Some(tree),
            mode,
            self.storage.git_sources(),
            LocalSourceLimits::default(),
        )
    }

    pub(super) fn advance(&self) -> (GitCommitId, GitTreeId) {
        std::fs::write(
            self.repository.join("main.omg"),
            b"machine Main::changed() {}\n",
        )
        .unwrap();
        run_test_git(&self.repository, ["add", "main.omg"]);
        run_test_git(
            &self.repository,
            ["commit", "--quiet", "-m", "advance source"],
        );
        let commit = run_test_git_with_input(&self.repository, ["rev-parse", "HEAD"], b"");
        let tree = run_test_git_with_input(&self.repository, ["rev-parse", "HEAD^{tree}"], b"");
        (
            GitCommitId::parse_hex(&commit).unwrap(),
            GitTreeId::parse_hex(&tree).unwrap(),
        )
    }

    pub(super) fn disconnect(&mut self) {
        let offline = self.repository.with_extension("offline");
        std::fs::rename(&self.repository, &offline).unwrap();
        self.repository = offline;
    }

    pub(super) fn entry(&self) -> PathBuf {
        self.request_entry(&self.request)
    }

    pub(super) fn request_entry(&self, request: &GitSourceRequest) -> PathBuf {
        let identity = git_cache_identity(
            request.locator_identity(),
            request.requested_revision(),
            request.execution_transport(),
        );
        self.storage
            .git_sources()
            .path()
            .join(format!("git-{identity}"))
    }

    pub(super) fn assert_original(&self, resolved: &ResolvedGitSource) {
        assert_eq!(resolved.commit(), self.commit.to_hex());
        assert_eq!(resolved.tree(), self.tree.to_hex());
        assert_eq!(resolved.materialized_tree(), self.tree.to_hex());
        assert_eq!(resolved.requested_revision(), "HEAD");
        assert!(resolved.selected_member().is_none());
        assert_eq!(
            std::fs::read(resolved.snapshot_root().join("main.omg")).unwrap(),
            b"machine Main::main() {}\n"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        remove_fixture(&self.repository, &self.storage_base);
    }
}

// Retry and cache-recovery tests for Git resolution across cold full-ID fetch
// failures, cold selector discovery failures, incomplete exact caches, and
// symlinked cache entries.

pub(in crate::git::tests::exact_revision) fn ordinary(
    fixture: &Fixture,
) -> Result<ResolvedGitSource, SourceResolveError> {
    resolve_git_source_in_lane(
        fixture.storage.primary_git().unwrap(),
        &fixture.request,
        fixture.storage.git_sources(),
        LocalSourceLimits::default(),
    )
}

pub(in crate::git::tests::exact_revision) fn reconnect(fixture: &mut Fixture, original: PathBuf) {
    std::fs::rename(&fixture.repository, &original).unwrap();
    fixture.repository = original;
}

pub(in crate::git::tests::exact_revision) fn assert_transport_failure(error: SourceResolveError) {
    assert!(matches!(error, SourceResolveError::Git { .. }), "{error:?}");
    assert!(!error.to_string().contains("source.identity"), "{error}");
}

#[test]
pub(in crate::git::tests::exact_revision) fn cold_full_id_fetch_failure_retries_without_manual_cache_changes()
 {
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
pub(in crate::git::tests::exact_revision) fn cold_selector_discovery_failure_does_not_publish_an_entry()
 {
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
pub(in crate::git::tests::exact_revision) fn incomplete_exact_cache_is_untouched_offline_and_rebuilt_only_at_recorded_pin()
 {
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
pub(in crate::git::tests::exact_revision) fn incomplete_entry_symlink_does_not_remove_its_target() {
    let fixture = Fixture::new("retry-entry-symlink");
    let external = fixture.repository.join("not-cache");
    std::fs::create_dir(&external).unwrap();
    let sentinel = external.join("keep");
    std::fs::write(&sentinel, b"not resolver state").unwrap();
    std::os::unix::fs::symlink(&external, fixture.entry()).unwrap();
    assert!(ordinary(&fixture).is_err());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"not resolver state");
}
