//! Whole-root operation-local pins never refresh a mutable selector.
use super::{
    GIT_CACHE_METADATA, GitSourceRequest, LocalSourceLimits, PathBuf, SourceResolveError,
    SourceResolver, create_git_source, git_cache_identity, local_git_request,
    make_tree_owner_writable, resolve_git_source_from_pin_in_lane, temp_root,
    test_system_git_executor,
};
use crate::PrimaryGitChoices;
use crate::observations::resolved::{GitAcquisitionPin, ResolvedGitSource};

struct Fixture {
    repository: PathBuf,
    storage_base: PathBuf,
    storage: SourceResolver,
    request: GitSourceRequest,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let (repository, _) = create_git_source(name);
        let request = local_git_request(&repository, "HEAD");
        let storage_base = temp_root(name);
        let primary_git = test_system_git_executor(request.execution_transport())
            .expect("select local fixture Git")
            .execution_backend
            .executable()
            .to_path_buf();
        let storage = SourceResolver::for_hardened_base(
            &storage_base,
            PrimaryGitChoices {
                explicit_git: Some(primary_git.as_ref()),
                ..PrimaryGitChoices::default()
            },
        )
        .expect("retain fixture storage");
        Self {
            repository,
            storage_base,
            storage,
            request,
        }
    }

    fn resolve(
        &self,
        pin: Option<&GitAcquisitionPin>,
    ) -> Result<ResolvedGitSource, SourceResolveError> {
        resolve_git_source_from_pin_in_lane(
            self.storage.primary_git().unwrap(),
            &self.request,
            pin,
            self.storage.git_sources(),
            LocalSourceLimits::default(),
        )
    }

    fn disconnect(&mut self) {
        let offline = self.repository.with_extension("offline");
        std::fs::rename(&self.repository, &offline).expect("remove fixture transport");
        self.repository = offline;
    }

    fn entry(&self) -> PathBuf {
        let identity = git_cache_identity(
            self.request.locator_identity(),
            self.request.requested_revision(),
            self.request.execution_transport(),
        );
        self.storage
            .git_sources()
            .path()
            .join(format!("git-{identity}"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.repository);
        make_tree_owner_writable(&self.storage_base);
        let _ = std::fs::remove_dir_all(&self.storage_base);
    }
}

#[test]
fn whole_root_pin_rejects_another_request_before_transport() {
    let mut fixture = Fixture::new("whole-root-pin-request");
    let first = fixture.resolve(None).expect("resolve original request");
    let pin = first.acquisition_pin();
    let different_revision = local_git_request(&fixture.repository, first.commit());
    fixture.disconnect();
    let different_locator = local_git_request(&fixture.repository, "HEAD");
    for request in [&different_revision, &different_locator] {
        let error = resolve_git_source_from_pin_in_lane(
            fixture.storage.primary_git().unwrap(),
            request,
            Some(&pin),
            fixture.storage.git_sources(),
            LocalSourceLimits::default(),
        )
        .expect_err("pin must match both locator and authored selector");
        assert_eq!(
            error,
            SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git acquisition reuse pin does not match the exact source request"
                    .to_owned(),
            }
        );
    }
}

#[test]
fn whole_root_pin_rejects_absent_cache_without_reacquisition() {
    let mut fixture = Fixture::new("whole-root-pin-absent");
    let pin = fixture
        .resolve(None)
        .expect("resolve initial request")
        .acquisition_pin();
    let entry = fixture.entry();
    let retired_entry = entry.with_extension("retired");
    std::fs::rename(&entry, &retired_entry).expect("remove exact acquisition cache entry");
    fixture.disconnect();

    assert_eq!(
        fixture
            .resolve(Some(&pin))
            .expect_err("missing pin custody must not fetch"),
        SourceResolveError::GitCacheInvalid {
            path: entry.clone(),
            message: "pinned Git acquisition cache entry is absent".to_owned(),
        }
    );
    assert!(
        !entry.exists(),
        "rejection must not recreate the cache entry"
    );
}

#[test]
fn whole_root_pin_rejects_corrupt_cache_and_changed_snapshot_without_fetch() {
    for change_snapshot in [false, true] {
        let mut fixture = Fixture::new("whole-root-pin-corruption");
        let first = fixture.resolve(None).expect("resolve initial request");
        let pin = first.acquisition_pin();
        let entry = fixture.entry();
        let altered = if change_snapshot {
            first.snapshot_root().join("main.omg")
        } else {
            entry.join(GIT_CACHE_METADATA)
        };
        let mut permissions = std::fs::metadata(&altered).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(permissions.mode() | 0o200);
        }
        // Off Unix there are no permission mode bits, so clearing the read-only
        // attribute is the only way to make the retained file writable again.
        // The lint's world-writable concern is a Unix one, and the Unix arm
        // above widens only the owner bit through `PermissionsExt::set_mode`.
        #[cfg(not(unix))]
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&altered, permissions).expect("make fixture file owner-writable");
        std::fs::write(&altered, b"changed cache contents\n")
            .expect("alter retained cache contents");
        fixture.disconnect();

        let error = fixture
            .resolve(Some(&pin))
            .expect_err("changed custody must not refetch");
        assert!(
            matches!(error, SourceResolveError::GitCacheInvalid { .. }),
            "{error:?}"
        );
        assert!(
            !entry.join(GIT_CACHE_METADATA).exists(),
            "bad cache custody is invalidated"
        );
    }
}
