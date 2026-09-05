//! Root selection must consume the same operation-local acquisition pin as a member.

use super::{make_tree_owner_writable, run_test_git, temp_root, test_git_head};
use crate::declarations::{BuildDeclarationKind, roles::BuildDeclarationError};
use crate::resolution::source::{
    GitPackageSourceRequest, ResolvePackageSourceError, ResolvedPackageSource,
    resolve_selected_git_package_source_from_pin_in_lanes,
    resolve_selected_git_project_source_from_pin_in_lanes,
};
use package_source::{
    GitAcquisitionPin, GitSourceRequest, LocalSourceLimits, ResolvedGitSource, SourceResolveError,
    SourceResolverStorage,
};
use std::path::PathBuf;

struct Fixture {
    root: PathBuf,
    repository: PathBuf,
}

impl Fixture {
    fn new(role: &str) -> Self {
        let root = temp_root("pinned-git-root");
        let repository = root.join("repository");
        std::fs::create_dir_all(&repository).unwrap();
        let fixture = Self { root, repository };
        fixture.write(role, "original-root");
        run_test_git(&fixture.repository, ["init", "--quiet"]);
        run_test_git(
            &fixture.repository,
            ["config", "user.email", "omega@example.invalid"],
        );
        run_test_git(&fixture.repository, ["config", "user.name", "Omega Tests"]);
        run_test_git(&fixture.repository, ["add", "."]);
        run_test_git(&fixture.repository, ["commit", "--quiet", "-m", "original"]);
        run_test_git(&fixture.repository, ["branch", "-M", "main"]);
        fixture
    }

    fn write(&self, role: &str, name: &str) {
        std::fs::write(
            self.repository.join("build.omg"),
            format!("machine build(builder: &mut Build) {{\n    builder.{role}(\"{name}\");\n}}\n"),
        )
        .unwrap();
        std::fs::write(
            self.repository.join("main.omg"),
            "machine Main::main() {}\n",
        )
        .unwrap();
    }

    fn request(&self, revision: &str) -> GitPackageSourceRequest {
        GitPackageSourceRequest::root(
            GitSourceRequest::for_local_test_repository_with_lineage(
                &self.repository,
                Some(revision.to_owned()),
                "https://github.com/CathedralOS/pinned-root-fixture.git",
            )
            .unwrap(),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        make_tree_owner_writable(&self.root);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn resolve(
    request: &GitPackageSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    storage: &SourceResolverStorage,
    application: bool,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let resolver = if application {
        resolve_selected_git_project_source_from_pin_in_lanes
    } else {
        resolve_selected_git_package_source_from_pin_in_lanes
    };
    resolver(
        request,
        pin,
        storage.git_sources(),
        storage.workspace_members(),
        LocalSourceLimits::default(),
    )
}

#[test]
fn root_selection_pin_retains_original_commit_and_declaration_after_branch_moves() {
    for (role, expected_role) in [
        ("package", BuildDeclarationKind::Package),
        ("application", BuildDeclarationKind::Application),
    ] {
        let fixture = Fixture::new(role);
        let storage = SourceResolverStorage::for_hardened_base(fixture.root.join("cache")).unwrap();
        let request = fixture.request("main");
        let application = role == "application";
        let original = resolve(&request, None, &storage, application).unwrap();
        let pin = original.source().acquisition_pin();
        let original_commit = test_git_head(&fixture.repository);
        assert_eq!(original.source().commit(), original_commit);

        fixture.write(role, "changed-root");
        run_test_git(&fixture.repository, ["add", "."]);
        run_test_git(&fixture.repository, ["commit", "--quiet", "-m", "changed"]);
        let changed_commit = test_git_head(&fixture.repository);
        assert_ne!(changed_commit, original_commit);

        let retained = resolve(&request, Some(&pin), &storage, application).unwrap();
        assert_eq!(retained.key(), original.key());
        assert_eq!(retained.key().name().as_str(), "original-root");
        assert_eq!(retained.role(), expected_role);
        assert_eq!(retained.resolution(), original.resolution());
        assert_eq!(retained.source().commit(), original_commit);
        assert_eq!(retained.source().tree(), original.source().tree());
        assert_eq!(retained.source().requested_revision(), "main");

        if application {
            assert!(matches!(
                resolve(&request, Some(&pin), &storage, false),
                Err(ResolvePackageSourceError::Declaration(
                    BuildDeclarationError::ExpectedPackageDeclaration {
                        found: BuildDeclarationKind::Application
                    }
                ))
            ));
        }

        let changed = resolve(&request, None, &storage, application).unwrap();
        assert_eq!(changed.key().name().as_str(), "changed-root");
        assert_eq!(changed.source().commit(), changed_commit);
    }
}

#[test]
fn root_selection_rejects_wrong_request_pin_and_reuses_matching_pin_offline() {
    let fixture = Fixture::new("package");
    let storage = SourceResolverStorage::for_hardened_base(fixture.root.join("cache")).unwrap();
    let request = fixture.request("main");
    let original = resolve(&request, None, &storage, false).unwrap();
    let pin = original.source().acquisition_pin();
    let wrong_request = fixture.request("HEAD");
    assert!(matches!(
        resolve(&wrong_request, Some(&pin), &storage, false),
        Err(ResolvePackageSourceError::Source(
            SourceResolveError::GitExecutionBoundaryInvalid { .. }
        ))
    ));

    // Neither matching reuse nor an absent-cache failure may refetch. The
    // exact fixture remote is unavailable for both operations below.
    std::fs::rename(&fixture.repository, fixture.root.join("offline-repository")).unwrap();
    let retained = resolve(&request, Some(&pin), &storage, false).unwrap();
    assert_eq!(retained.key(), original.key());
    assert_eq!(retained.resolution(), original.resolution());
    let empty = SourceResolverStorage::for_hardened_base(fixture.root.join("empty-cache")).unwrap();
    assert!(
        matches!(resolve(&request, Some(&pin), &empty, false), Err(ResolvePackageSourceError::Source(SourceResolveError::GitCacheInvalid { message, .. })) if message == "pinned Git acquisition cache entry is absent")
    );
}
