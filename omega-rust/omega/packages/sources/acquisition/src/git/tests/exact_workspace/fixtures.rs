use super::*;

pub(super) struct Fixture {
    pub(super) repository: PathBuf,
    storage_base: PathBuf,
    pub(super) storage: SourceResolverStorage,
    pub(super) request: GitSourceRequest,
    pub(super) commit: GitCommitId,
    pub(super) root_tree: GitTreeId,
    pub(super) member_tree: GitTreeId,
}

pub(super) fn limits() -> LocalSourceLimits {
    LocalSourceLimits {
        max_entries: 32,
        max_bytes: 1024,
        max_depth: 8,
    }
}
pub(super) fn declaration_limits() -> GitWorkspaceDeclarationLimits {
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
        let executable = test_system_git_executor(request.execution_transport())
            .unwrap()
            .execution_backend
            .executable()
            .to_path_buf();
        let storage_base = temp_root(name);
        let storage =
            SourceResolverStorage::for_hardened_base_with_primary_git(&storage_base, executable)
                .unwrap();
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
        let _ = std::fs::remove_dir_all(&self.repository);
        make_tree_owner_writable(&self.storage_base);
        let _ = std::fs::remove_dir_all(&self.storage_base);
    }
}
