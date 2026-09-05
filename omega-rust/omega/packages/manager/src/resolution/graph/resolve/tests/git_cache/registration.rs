use super::*;
use crate::resolution::graph::resolve::dependencies::resolve_registered_package_closure;
use crate::resolution::source::{GitPackageSourceRequest, ResolvedPackageSource};
use package_source::ResolvedGitSource;

const LOCATOR: &str = "https://github.com/CathedralOS/registration.git";

struct Fixture {
    repository: PathBuf,
    cache: PathBuf,
    storage: SourceResolverStorage,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let repository = temp_root(&format!("git-registration-{name}-repository"));
        let cache = temp_root(&format!("git-registration-{name}-cache"));
        write_package(
            &repository.join("packages/first"),
            "first",
            Some("../third"),
        );
        write_package(&repository.join("packages/second"), "second", None);
        write_package(&repository.join("packages/third"), "third", None);
        std::fs::write(
            repository.join("build.omg"),
            "machine build(builder: &mut Build) {\n    builder.member(\"packages/first\");\n    builder.member(\"packages/second\");\n    builder.member(\"packages/third\");\n}\n",
        )
        .unwrap();
        run_test_git(&repository, ["init", "--quiet"]);
        run_test_git(
            &repository,
            ["config", "user.email", "omega@example.invalid"],
        );
        run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
        run_test_git(&repository, ["add", "."]);
        run_test_git(&repository, ["commit", "--quiet", "-m", "workspace"]);
        run_test_git(&repository, ["branch", "accepted"]);
        run_test_git(&repository, ["branch", "alternate"]);
        let storage = SourceResolverStorage::for_hardened_base(&cache).unwrap();
        Self {
            repository,
            cache,
            storage,
        }
    }

    fn request(&self, locator: &str, revision: &str, member: &str) -> GitPackageSourceRequest {
        GitPackageSourceRequest::new(
            GitSourceRequest::for_local_test_repository_with_lineage(
                &self.repository,
                Some(revision.to_owned()),
                locator,
            )
            .unwrap(),
            crate::declarations::PackageSelection::Named(
                crate::declarations::PackageName::parse(member).unwrap(),
            ),
        )
    }

    fn resolve(
        &self,
        acquisitions: &mut GitAcquisitionCache<'_>,
        request: &GitPackageSourceRequest,
    ) -> ResolvedPackageSource<ResolvedGitSource> {
        acquisitions
            .resolve_selected(
                request,
                SourceCacheLane::Retained(self.storage.git_sources()),
                SourceCacheLane::Retained(self.storage.workspace_members()),
                LocalSourceLimits::default(),
            )
            .unwrap()
    }

    fn advance(&self) {
        std::fs::write(
            self.repository.join("packages/third/drift.omg"),
            "machine drift() {}\n",
        )
        .unwrap();
        run_test_git(&self.repository, ["add", "."]);
        run_test_git(&self.repository, ["commit", "--quiet", "-m", "move branch"]);
        run_test_git(&self.repository, ["branch", "--force", "alternate", "HEAD"]);
    }
}

impl Drop for Fixture {
    #[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
    fn drop(&mut self) {
        let mut pending = vec![self.repository.clone(), self.cache.clone()];
        while let Some(path) = pending.pop() {
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            #[cfg(unix)]
            if metadata.is_dir() {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700));
            }
            #[cfg(windows)]
            {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(false);
                let _ = std::fs::set_permissions(&path, permissions);
            }
            if metadata.is_dir()
                && let Ok(entries) = std::fs::read_dir(path)
            {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
        }
        let _ = std::fs::remove_dir_all(&self.repository);
        let _ = std::fs::remove_dir_all(&self.cache);
    }
}

#[test]
fn distinct_selectors_at_one_commit_preserve_first_context_and_path_pin() {
    same_commit_registration(LOCATOR, "alternate");
}

#[test]
fn distinct_locators_at_one_commit_preserve_first_context_and_path_pin() {
    same_commit_registration("git@github.com:CathedralOS/registration.git", "accepted");
}

fn same_commit_registration(locator: &str, revision: &str) {
    let fixture = Fixture::new(revision);
    let mut acquisitions = GitAcquisitionCache::default();
    let first_request = fixture.request(LOCATOR, "accepted", "first");
    let second_request = fixture.request(locator, revision, "second");
    assert_ne!(first_request.acquisition(), second_request.acquisition());
    assert_eq!(
        first_request.acquisition().lineage(),
        second_request.acquisition().lineage()
    );
    let first = fixture.resolve(&mut acquisitions, &first_request);
    let second = fixture.resolve(&mut acquisitions, &second_request);
    assert_eq!(first.resolution(), second.resolution());

    for reversed in [false, true] {
        let mut workspaces = BTreeMap::new();
        let registrations = if reversed {
            [(&second_request, &second), (&first_request, &first)]
        } else {
            [(&first_request, &first), (&second_request, &second)]
        };
        let mut original = None;
        for (request, source) in registrations {
            register_git_repository(
                &mut workspaces,
                request.acquisition(),
                source.key().source_lineage(),
                source.resolution(),
                source.selection_evidence(),
                source.source_limits(),
            )
            .expect("equivalent repository state accepts another authored request");
            if let Some(original) = &original {
                assert_eq!(
                    &workspaces, original,
                    "first context and acquisition request are retained"
                );
            } else {
                original = Some(workspaces.clone());
            }
        }

        // Move both selectors after registration. The not-yet-selected Path
        // sibling must still come from the first context's cached acquisition.
        if !reversed {
            fixture.advance();
            run_test_git(
                &fixture.repository,
                ["branch", "--force", "accepted", "HEAD"],
            );
        }
        let closure = resolve_registered_package_closure(
            PackageRootSourceRequest::Git(first_request.clone()),
            first.clone().into_custody(),
            PackageSourceClosureLimits::default(),
            SourceCacheLane::Retained(fixture.storage.workspace_members()),
            SourceCacheLane::Retained(fixture.storage.git_sources()),
            SourceCacheLane::Retained(fixture.storage.external_local_sources()),
            LocalSourceLimits::default(),
            &mut workspaces,
            &mut BTreeMap::new(),
            None,
            &mut acquisitions,
        )
        .expect("Path sibling uses the registered repository revision");
        assert_eq!(closure.custodies().len(), 2);
        let third = closure
            .custodies()
            .iter()
            .find(|source| source.key().name().as_str() == "third")
            .unwrap();
        assert_eq!(third.resolution(), first.resolution());
        assert!(!third.snapshot_root().join("drift.omg").exists());
    }
}

#[test]
fn distinct_commits_reject_in_both_registration_orders() {
    let fixture = Fixture::new("conflicting-commits");
    let mut acquisitions = GitAcquisitionCache::default();
    let first_request = fixture.request(LOCATOR, "accepted", "first");
    let first = fixture.resolve(&mut acquisitions, &first_request);
    fixture.advance();
    let second_request = fixture.request(LOCATOR, "alternate", "second");
    let second = fixture.resolve(&mut acquisitions, &second_request);
    assert_ne!(first.resolution(), second.resolution());
    for registrations in [
        [(&first_request, &first), (&second_request, &second)],
        [(&second_request, &second), (&first_request, &first)],
    ] {
        let mut workspaces = BTreeMap::new();
        let (request, source) = registrations[0];
        let identity = register_git_repository(
            &mut workspaces,
            request.acquisition(),
            source.key().source_lineage(),
            source.resolution(),
            source.selection_evidence(),
            source.source_limits(),
        )
        .unwrap();
        let original = workspaces.clone();
        let (request, source) = registrations[1];
        let error = register_git_repository(
            &mut workspaces,
            request.acquisition(),
            source.key().source_lineage(),
            source.resolution(),
            source.selection_evidence(),
            source.source_limits(),
        )
        .expect_err("one lineage cannot register different immutable revisions");
        assert!(
            matches!(error, ResolveDependencySourceError::ConflictingWorkspaceRoot { identity: actual } if actual == identity)
        );
        assert_eq!(workspaces, original, "conflict retains the first context");
    }
}

#[test]
fn workspace_evidence_and_source_limits_still_require_equality() {
    let fixture = Fixture::new("conflicting-evidence");
    let mut acquisitions = GitAcquisitionCache::default();
    let request = fixture.request(LOCATOR, "accepted", "first");
    let source = fixture.resolve(&mut acquisitions, &request);
    let mut workspaces = BTreeMap::new();
    register_git_repository(
        &mut workspaces,
        request.acquisition(),
        source.key().source_lineage(),
        source.resolution(),
        source.selection_evidence(),
        source.source_limits(),
    )
    .unwrap();
    let original = workspaces.clone();
    let tighter_limits = LocalSourceLimits {
        max_entries: source.source_limits().max_entries - 1,
        ..source.source_limits()
    };
    for (evidence, limits) in [
        (
            &crate::resolution::source::PackageSourceSelectionEvidence::Root,
            source.source_limits(),
        ),
        (source.selection_evidence(), tighter_limits),
    ] {
        let error = register_git_repository(
            &mut workspaces,
            request.acquisition(),
            source.key().source_lineage(),
            source.resolution(),
            evidence,
            limits,
        )
        .expect_err("request relaxation must preserve repository evidence and limits");
        assert!(matches!(
            error,
            ResolveDependencySourceError::ConflictingWorkspaceRoot { .. }
        ));
        assert_eq!(workspaces, original);
    }
}
