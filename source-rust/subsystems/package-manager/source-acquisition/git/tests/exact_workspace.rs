// Exact-revision Git workspace projection tests verifying that recorded root
// objects select authenticated member trees without materializing unrelated
// payloads or bypassing declaration limits on warm recovery.

use super::{
    GitCommitId, GitExactRevisionAcquisition, GitSourceRequest, GitTreeId, GitWorkspaceDeclaration,
    GitWorkspaceDeclarationLimits, GitWorkspaceProjectionError, GitWorkspaceProjectionPlanner,
    GitWorkspaceProjectionResult, GitWorkspaceSelection, LocalSourceLimits, PathBuf,
    SourceRelativePath, SourceResolveError, SourceResolver, create_git_source, fixture_storage,
    local_git_request, remove_fixture, resolve_git_workspace_member_at_revision_in_lanes,
    run_test_git, run_test_git_with_input, temp_root,
};

#[test]
fn exact_old_workspace_declarations_survive_branch_drift_and_offline_recovery() {
    let mut fixture = Fixture::new("exact-workspace-drift");
    fixture.advance();
    let mut planner = Planner::default();
    let first = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch, &mut planner)
        .unwrap();
    fixture.assert_original(&first);
    assert_eq!((planner.discoveries, planner.selections), (1, 1));
    assert_eq!(
        *first.evidence(),
        "selected from exact authenticated declarations"
    );
    fixture.disconnect();
    for mode in [
        GitExactRevisionAcquisition::Offline,
        GitExactRevisionAcquisition::AllowFetch,
    ] {
        let recovered = fixture.resolve(mode, &mut planner).unwrap();
        fixture.assert_original(&recovered);
        assert_eq!(
            recovered.source().content_identity(),
            first.source().content_identity()
        );
    }
    assert_eq!((planner.discoveries, planner.selections), (3, 3));
}

#[test]
fn exact_workspace_rejects_selection_outside_authenticated_discovered_members() {
    let fixture = Fixture::new("exact-workspace-undiscovered");
    let mut planner = Planner {
        select_undiscovered: true,
        ..Planner::default()
    };
    let error = fixture
        .resolve(GitExactRevisionAcquisition::AllowFetch, &mut planner)
        .unwrap_err();
    assert_eq!((planner.discoveries, planner.selections), (1, 1));
    assert_eq!(
        error,
        GitWorkspaceProjectionError::Source(SourceResolveError::GitTreeInvalid {
            path: b"packages/other".to_vec(),
            message: "workspace planner selected a member absent from its discovered set".into(),
        })
    );
}

#[test]
fn warm_exact_workspace_reapplies_declaration_limits_before_planning() {
    let mut fixture = Fixture::new("exact-workspace-declaration-limit");
    fixture
        .resolve(
            GitExactRevisionAcquisition::AllowFetch,
            &mut Planner::default(),
        )
        .unwrap();
    fixture.disconnect();
    let mut planner = Planner::default();
    let error = resolve_git_workspace_member_at_revision_in_lanes(
        fixture.storage.primary_git().unwrap(),
        &fixture.request,
        &fixture.commit,
        &fixture.root_tree,
        GitExactRevisionAcquisition::Offline,
        fixture.storage.git_sources(),
        fixture.storage.workspace_members(),
        limits(),
        GitWorkspaceDeclarationLimits::new(8, 8, 8),
        &mut planner,
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            GitWorkspaceProjectionError::Source(SourceResolveError::TooManyBytes { limit: 8 })
        ),
        "{error:?}"
    );
    assert_eq!((planner.discoveries, planner.selections), (0, 0));
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

    pub(super) fn resolve(
        &self,
        mode: GitExactRevisionAcquisition,
        planner: &mut Planner,
    ) -> Result<GitWorkspaceProjectionResult<&'static str>, GitWorkspaceProjectionError<&'static str>>
    {
        resolve_git_workspace_member_at_revision_in_lanes(
            self.storage.primary_git().unwrap(),
            &self.request,
            &self.commit,
            &self.root_tree,
            mode,
            self.storage.git_sources(),
            self.storage.workspace_members(),
            limits(),
            declaration_limits(),
            planner,
        )
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

    pub(super) fn assert_original(&self, result: &GitWorkspaceProjectionResult<&str>) {
        let source = result.source();
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

// Deterministic workspace projection planner used by exact-workspace tests to
// verify that discovery and selection see exact historical declarations.

#[derive(Default)]
struct Planner {
    pub(super) discoveries: usize,
    pub(super) selections: usize,
    pub(super) select_undiscovered: bool,
}

impl GitWorkspaceProjectionPlanner for Planner {
    type Error = &'static str;
    type Evidence = &'static str;

    fn discover_members(
        &mut self,
        root: &GitWorkspaceDeclaration,
    ) -> Result<Vec<SourceRelativePath>, Self::Error> {
        self.discoveries += 1;
        if root.bytes() != b"old workspace declaration\n" {
            return Err("root declaration drifted from exact old tree");
        }
        Ok(vec![SourceRelativePath::parse("packages/member").unwrap()])
    }

    fn select_member(
        &mut self,
        root: &GitWorkspaceDeclaration,
        members: &[GitWorkspaceDeclaration],
    ) -> Result<GitWorkspaceSelection<Self::Evidence>, Self::Error> {
        self.selections += 1;
        let [member] = members else {
            return Err("one authenticated member expected");
        };
        if root.bytes() != b"old workspace declaration\n"
            || member.bytes() != b"old member declaration\n"
            || member.member_path().map(SourceRelativePath::as_str) != Some("packages/member")
        {
            return Err("member declaration drifted from exact old tree");
        }
        Ok(GitWorkspaceSelection::new(
            SourceRelativePath::parse(if self.select_undiscovered {
                "packages/other"
            } else {
                "packages/member"
            })
            .unwrap(),
            "selected from exact authenticated declarations",
        ))
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
        let result = fixture.resolve(
            GitExactRevisionAcquisition::AllowFetch,
            &mut Planner::default(),
        );
        assert!(matches!(
            result,
            Err(GitWorkspaceProjectionError::Source(
                SourceResolveError::GitSubmodulesUnsupported { .. }
            ))
        ));
    }
}
