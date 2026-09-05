//! Whole-root operation-local pins never refresh a mutable selector.

use super::*;
use crate::observations::resolved::{GitAcquisitionPin, ResolvedGitSource};

struct Fixture {
    repository: PathBuf,
    storage_base: PathBuf,
    storage: SourceResolverStorage,
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
        let storage =
            SourceResolverStorage::for_hardened_base_with_primary_git(&storage_base, primary_git)
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
fn whole_root_pin_keeps_original_commit_and_tree_after_selector_refresh() {
    let mut fixture = Fixture::new("whole-root-pin-movement");
    let first = fixture.resolve(None).expect("resolve initial HEAD");
    let pin = first.acquisition_pin();
    std::fs::write(
        fixture.repository.join("main.omg"),
        b"machine Main::changed() {}\n",
    )
    .expect("advance authored source");
    run_test_git(&fixture.repository, ["add", "main.omg"]);
    run_test_git(
        &fixture.repository,
        ["commit", "--quiet", "-m", "advance HEAD"],
    );
    let advanced_commit = run_test_git_with_input(&fixture.repository, ["rev-parse", "HEAD"], b"");

    let pinned = fixture
        .resolve(Some(&pin))
        .expect("pin ignores branch movement");
    assert_eq!(pinned, first);
    let refreshed = fixture
        .resolve(None)
        .expect("None refreshes the current selector");
    assert_eq!(refreshed.commit(), advanced_commit);
    assert_ne!(refreshed.commit(), first.commit());
    assert_ne!(refreshed.tree(), first.tree());
    assert_eq!(
        std::fs::read(refreshed.snapshot_root().join("main.omg")).expect("read refreshed source"),
        b"machine Main::changed() {}\n"
    );

    fixture.disconnect();
    let offline = fixture
        .resolve(Some(&pin))
        .expect("pin reuses old objects without transport");
    assert_eq!(offline.commit(), first.commit());
    assert_eq!(offline.tree(), first.tree());
    assert_eq!(offline.materialized_tree(), first.tree());
    assert_eq!(offline.content_identity(), first.content_identity());
    assert_eq!(offline.lineage(), first.lineage());
    assert_eq!(offline.requested_revision(), "HEAD");
    assert!(offline.selected_member().is_none());
    assert_eq!(
        std::fs::read(offline.snapshot_root().join("main.omg")).expect("read pinned source"),
        b"machine Main::main() {}\n"
    );
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
        #[cfg(not(unix))]
        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "Windows read-only attributes do not change Unix permission bits"
        )]
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

#[test]
fn whole_root_pin_still_enforces_current_source_limits() {
    let mut fixture = Fixture::new("whole-root-pin-limits");
    let pin = fixture
        .resolve(None)
        .expect("resolve initial request")
        .acquisition_pin();
    fixture.disconnect();
    let error = resolve_git_source_from_pin_in_lane(
        &fixture.request,
        Some(&pin),
        fixture.storage.git_sources(),
        LocalSourceLimits {
            max_bytes: 0,
            ..LocalSourceLimits::default()
        },
    )
    .expect_err("pin cannot retain previous, looser limits");
    assert_eq!(error, SourceResolveError::TooManyBytes { limit: 0 });
}
