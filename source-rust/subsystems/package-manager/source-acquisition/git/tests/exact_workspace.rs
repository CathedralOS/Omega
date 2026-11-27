// Exact-revision Git workspace session tests verifying that recorded root
// objects select authenticated member trees without materializing unrelated
// payloads or bypassing declaration limits on warm recovery.

use super::{
    GitCommitId, GitExactRevisionAcquisition, GitSourceRequest, GitTreeId,
    GitWorkspaceDeclarationLimits, GitWorkspaceSession, LocalSourceLimits, PathBuf,
    SourceRelativePath, SourceResolveError, SourceResolver, create_git_source, fixture_storage,
    local_git_request, remove_fixture, run_test_git, run_test_git_with_input, temp_root,
};
use crate::ResolvedGitSource;

#[test]
fn exact_old_workspace_declarations_survive_branch_drift_and_offline_recovery() {
    let mut fixture = Fixture::new("exact-workspace-drift");
    fixture.advance();
    let first = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch, &member())
        .unwrap();
    fixture.assert_original(&first);
    fixture.disconnect();
    for mode in [
        GitExactRevisionAcquisition::Offline,
        GitExactRevisionAcquisition::AllowFetch,
    ] {
        let recovered = fixture.resolve(mode, &member()).unwrap();
        fixture.assert_original(&recovered);
        assert_eq!(recovered.content_identity(), first.content_identity());
    }
}

#[test]
fn exact_workspace_rejects_selection_outside_authenticated_declared_members() {
    let fixture = Fixture::new("exact-workspace-undeclared");
    let other = SourceRelativePath::parse("packages/other").unwrap();
    let error = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch, &other)
        .unwrap_err();
    assert_eq!(
        error,
        SourceResolveError::GitTreeInvalid {
            path: b"packages/other".to_vec(),
            message: "selected member is absent from the acquired declaration roster".into(),
        }
    );
}

#[test]
fn warm_exact_workspace_reapplies_declaration_limits_before_member_requests() {
    let mut fixture = Fixture::new("exact-workspace-limit");
    fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch, &member())
        .unwrap();
    fixture.disconnect();
    let error = GitWorkspaceSession::open_at_revision(
        &fixture.storage,
        &fixture.request,
        &fixture.commit,
        Some(&fixture.root_tree),
        GitExactRevisionAcquisition::Offline,
        limits(),
        GitWorkspaceDeclarationLimits::new(8, 8, 8),
    )
    .unwrap_err();
    assert!(
        matches!(error, SourceResolveError::TooManyBytes { limit: 8 }),
        "{error:?}"
    );
}

// Test fixture harness for exact-revision Git workspace member resolution,
// maintaining persisted root and member tree IDs across repository drift.

struct Fixture {
    pub(super) repository: PathBuf,
    storage_base: PathBuf,
    pub(super) storage: SourceResolver,
    pub(super) request: GitSourceRequest,
    pub(super) commit: GitCommitId,
    pub(super) root_tree: GitTreeId,
    pub(super) member_tree: GitTreeId,
}

fn limits() -> LocalSourceLimits {
    LocalSourceLimits {
        max_entries: 32,
        max_bytes: 1024,
        max_depth: 8,
    }
}
fn declaration_limits() -> GitWorkspaceDeclarationLimits {
    GitWorkspaceDeclarationLimits::new(8, 256, 512)
}
fn member() -> SourceRelativePath {
    SourceRelativePath::parse("packages/member").unwrap()
}

impl Fixture {
    pub(super) fn new(name: &str) -> Self {
        let (repository, _) = create_git_source(name);
        std::fs::create_dir_all(repository.join("packages/member/source")).unwrap();
        for (path, bytes) in [
            ("build.omg", b"old workspace declaration\n".as_slice()),
            (
                "packages/member/build.omg",
                b"old member declaration\n".as_slice(),
            ),
            (
                "packages/member/source/lib.omg",
                b"machine Member::old() {}\n".as_slice(),
            ),
        ] {
            std::fs::write(repository.join(path), bytes).unwrap();
        }
        std::fs::write(repository.join("unrelated.bin"), vec![b'x'; 4096]).unwrap();
        run_test_git(&repository, ["add", "."]);
        run_test_git(&repository, ["commit", "--quiet", "-m", "old workspace"]);
        let commit = run_test_git_with_input(&repository, ["rev-parse", "HEAD"], b"");
        let root_tree = run_test_git_with_input(&repository, ["rev-parse", "HEAD^{tree}"], b"");
        let member_tree =
            run_test_git_with_input(&repository, ["rev-parse", "HEAD:packages/member"], b"");
        let request = local_git_request(&repository, "HEAD");
        let storage_base = temp_root(name);
        let storage = fixture_storage(&storage_base, &request);
        Self {
            repository,
            storage_base,
            storage,
            request,
            commit: GitCommitId::parse_hex(&commit).unwrap(),
            root_tree: GitTreeId::parse_hex(&root_tree).unwrap(),
            member_tree: GitTreeId::parse_hex(&member_tree).unwrap(),
        }
    }

    // Open the recorded root, request the one declared member, and materialize `selected`.
    pub(super) fn resolve(
        &self,
        mode: GitExactRevisionAcquisition,
        selected: &SourceRelativePath,
    ) -> Result<ResolvedGitSource, SourceResolveError> {
        let session = GitWorkspaceSession::open_at_revision(
            &self.storage,
            &self.request,
            &self.commit,
            Some(&self.root_tree),
            mode,
            limits(),
            declaration_limits(),
        )?;
        assert_eq!(
            session.root_declaration().bytes(),
            b"old workspace declaration\n"
        );
        session
            .open_members(&self.storage, &[member()])?
            .materialize(&self.storage, selected)
    }

    pub(super) fn advance(&self) {
        for path in [
            "build.omg",
            "packages/member/build.omg",
            "packages/member/source/lib.omg",
        ] {
            std::fs::write(
                self.repository.join(path),
                b"changed declaration or source\n",
            )
            .unwrap();
        }
        run_test_git(&self.repository, ["add", "."]);
        run_test_git(
            &self.repository,
            ["commit", "--quiet", "-m", "change workspace"],
        );
    }

    pub(super) fn disconnect(&mut self) {
        let offline = self.repository.with_extension("offline");
        std::fs::rename(&self.repository, &offline).unwrap();
        self.repository = offline;
    }

    pub(super) fn assert_original(&self, source: &ResolvedGitSource) {
        assert_eq!(source.commit(), self.commit.to_hex());
        assert_eq!(source.tree(), self.root_tree.to_hex());
        assert_eq!(source.materialized_tree(), self.member_tree.to_hex());
        assert_ne!(source.tree(), source.materialized_tree());
        assert_eq!(source.requested_revision(), "HEAD");
        assert_eq!(source.lineage(), self.request.lineage());
        assert_eq!(
            source.selected_member().unwrap().as_str(),
            "packages/member"
        );
        let projection = source.workspace_projection().unwrap();
        assert_eq!(projection.selected_member_tree(), self.member_tree.to_hex());
        assert_eq!(
            projection.root_declaration().bytes(),
            b"old workspace declaration\n"
        );
        assert_eq!(
            projection.member_declarations()[0].bytes(),
            b"old member declaration\n"
        );
        assert_eq!(
            std::fs::read(source.snapshot_root().join("source/lib.omg")).unwrap(),
            b"machine Member::old() {}\n"
        );
        assert!(!source.snapshot_root().join("unrelated.bin").exists());
        assert!(!source.snapshot_root().join("packages").exists());
        assert!(
            source
                .snapshot_root()
                .starts_with(self.storage.workspace_members().path())
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        remove_fixture(&self.repository, &self.storage_base);
    }
}

// Submodule rejection tests for exact-revision Git workspace projection,
// verifying that `.gitmodules` manifests or gitlinks inside the selected member
// reject even when root-level gitlinks remain inert.

pub(in crate::git::tests::exact_workspace) const MISSING_COMMIT: &str =
    "1111111111111111111111111111111111111111";

pub(in crate::git::tests::exact_workspace) fn add_submodule(
    fixture: &mut Fixture,
    manifest: &str,
    link: &str,
) {
    std::fs::write(
        fixture.repository.join(manifest),
        b"[submodule \"unavailable\"]\n",
    )
    .unwrap();
    run_test_git(&fixture.repository, ["add", manifest]);
    run_test_git(
        &fixture.repository,
        [
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            MISSING_COMMIT,
            link,
        ],
    );
    run_test_git(
        &fixture.repository,
        ["commit", "--quiet", "-m", "inert submodule edge"],
    );
    fixture.commit = GitCommitId::parse_hex(&run_test_git_with_input(
        &fixture.repository,
        ["rev-parse", "HEAD"],
        b"",
    ))
    .unwrap();
    fixture.root_tree = GitTreeId::parse_hex(&run_test_git_with_input(
        &fixture.repository,
        ["rev-parse", "HEAD^{tree}"],
        b"",
    ))
    .unwrap();
    fixture.member_tree = GitTreeId::parse_hex(&run_test_git_with_input(
        &fixture.repository,
        ["rev-parse", "HEAD:packages/member"],
        b"",
    ))
    .unwrap();
}

#[test]
pub(in crate::git::tests::exact_workspace) fn selected_member_submodule_metadata_or_gitlink_rejects()
 {
    for (manifest, link) in [
        ("packages/member/.gitmodules", "unavailable"),
        (".gitmodules", "packages/member/unavailable"),
    ] {
        let mut fixture = Fixture::new("workspace-selected-submodule");
        add_submodule(&mut fixture, manifest, link);
        let result = fixture.resolve(GitExactRevisionAcquisition::AllowFetch, &member());
        assert!(matches!(
            result,
            Err(SourceResolveError::GitSubmodulesUnsupported { .. })
        ));
    }
}
